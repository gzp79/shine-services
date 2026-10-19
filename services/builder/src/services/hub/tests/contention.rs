//! Heavy concurrency: many parallel clients on one hub, and two hubs (two service instances) sharing
//! one Redis registry.

use crate::{
    models::messages::{DisconnectReason, HubEvent, TopicKey},
    services::hub_test_utils::{
        assert_consistent, barrier, closed_with, connect, connect_on, count_connects, create_test_hub, dropped, drops,
        egress_state, find_registry_connection, lifecycle_by_user, lifecycle_of, open, payload, replay, spawn_reader,
        wait_event, Egress, EgressState, Lifecycle, Rng, TestHub, TestHubOptions, BARRIER_TIMEOUT,
    },
    services::HubService,
};
use shine_test::test;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle, time::timeout};
use uuid::Uuid;

// --- Parallel contention ------------------------------------------------------------------------

/// The expected outcome of one user's command script: commands of one sender are processed in order,
/// so a user driven by a single task has a fully predictable lifecycle.
#[derive(Default)]
struct UserModel {
    active: Option<Uuid>,
    lifecycle: Vec<Lifecycle>,
    notices: HashMap<Uuid, DisconnectReason>,
    dropped: BTreeMap<DisconnectReason, u64>,
}

impl UserModel {
    fn connect(&mut self, connection_id: Uuid) {
        if let Some(old) = self.active.take() {
            self.drop_active(old, DisconnectReason::Replaced);
        }
        self.lifecycle.push(Lifecycle::Connected(connection_id));
        self.active = Some(connection_id);
    }

    fn disconnect(&mut self, connection_id: Uuid) {
        if self.active == Some(connection_id) {
            self.active = None;
            self.lifecycle.push(Lifecycle::Disconnected(connection_id));
        }
    }

    fn drop(&mut self, connection_id: Uuid, reason: DisconnectReason) {
        if self.active == Some(connection_id) {
            self.active = None;
            self.drop_active(connection_id, reason);
        }
    }

    fn drop_active(&mut self, connection_id: Uuid, reason: DisconnectReason) {
        self.lifecycle.push(Lifecycle::Disconnected(connection_id));
        self.notices.insert(connection_id, reason);
        *self.dropped.entry(reason).or_default() += 1;
    }
}

const DROP_REASONS: [DisconnectReason; 2] = [DisconnectReason::SessionInvalid, DisconnectReason::HeartbeatInactive];

/// Runs a random but seeded script of connects, disconnects, forced drops and vanishing clients for one
/// user, returning the expected outcome and the egress channels that were kept.
async fn run_user_script(hub: HubService, user_id: Uuid, seed: u64, steps: usize) -> (UserModel, Vec<(Uuid, Egress)>) {
    let mut rng = Rng(seed);
    let mut model = UserModel::default();
    let mut owned = vec![];
    let mut receivers = HashMap::new();

    for _ in 0..steps {
        let stale = (!owned.is_empty()).then(|| owned[rng.below(owned.len() as u64) as usize]);
        match (rng.below(10), model.active) {
            (0..=3, _) | (_, None) => {
                let (connection_id, rx) = connect(&hub, user_id, 8);
                model.connect(connection_id);
                owned.push(connection_id);
                receivers.insert(connection_id, rx);
            }
            (4 | 5, Some(active)) => {
                hub.request_disconnection(user_id, active).unwrap();
                model.disconnect(active);
            }
            (6, Some(_)) => {
                let stale = stale.unwrap();
                hub.request_disconnection(user_id, stale).unwrap();
                model.disconnect(stale);
            }
            (7, Some(active)) => {
                let reason = DROP_REASONS[rng.below(2) as usize];
                hub.request_drop(user_id, active, reason).unwrap();
                model.drop(active, reason);
            }
            (8, Some(active)) => {
                // The client goes away: its socket is gone before the disconnect reaches the hub.
                receivers.remove(&active);
                hub.request_disconnection(user_id, active).unwrap();
                model.disconnect(active);
            }
            (_, Some(_)) => {
                let stale = stale.unwrap();
                hub.request_drop(user_id, stale, DisconnectReason::HeartbeatInactive)
                    .unwrap();
                model.drop(stale, DisconnectReason::HeartbeatInactive);
            }
        }
        if rng.below(3) == 0 {
            tokio::task::yield_now().await;
        }
    }

    (model, receivers.into_iter().collect())
}

#[test(serial = "hub-service")]
async fn parallel_users_each_follow_their_own_command_order() {
    let Some(TestHub { hub, registry, mut events, .. }) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    const USERS: u64 = 48;
    const STEPS: usize = 40;

    let users: Vec<(Uuid, u64)> = (0..USERS).map(|seed| (Uuid::new_v4(), 0x5EED_0000 + seed)).collect();
    let tasks: Vec<JoinHandle<_>> = users
        .iter()
        .map(|&(user_id, seed)| tokio::spawn(run_user_script(hub.clone(), user_id, seed, STEPS)))
        .collect();
    let mut outcomes = vec![];
    for task in tasks {
        outcomes.push(task.await.unwrap());
    }
    let seen = barrier(&hub, &mut events).await;

    let lifecycle = lifecycle_by_user(&seen);
    let active = replay(&seen);
    let mut expected_active = HashMap::new();
    let mut expected_dropped = BTreeMap::<DisconnectReason, u64>::new();
    for (&(user_id, seed), (model, receivers)) in users.iter().zip(&mut outcomes) {
        assert_eq!(
            lifecycle.get(&user_id).map(Vec::as_slice).unwrap_or_default(),
            model.lifecycle,
            "lifecycle of {user_id} (seed {seed:#x})"
        );
        if let Some(connection_id) = model.active {
            expected_active.insert(user_id, connection_id);
        }
        for (reason, count) in &model.dropped {
            *expected_dropped.entry(*reason).or_default() += count;
        }
        for (connection_id, rx) in receivers.iter_mut() {
            let expected = if model.active == Some(*connection_id) {
                open()
            } else {
                closed_with(model.notices.get(connection_id).copied())
            };
            assert_eq!(egress_state(rx), expected, "egress of {connection_id} (seed {seed:#x})");
        }
    }
    assert_eq!(active, expected_active);
    assert_eq!(drops(&seen), expected_dropped);
    assert_consistent(&hub, &registry, users.iter().map(|(u, _)| *u), &active).await;

    for (user_id, connection_id) in &active {
        hub.request_disconnection(*user_id, *connection_id).unwrap();
    }
    barrier(&hub, &mut events).await;
    assert_eq!(hub.stats().await.connections, 0);
}

#[test(serial = "hub-service")]
async fn same_user_contention_keeps_a_single_active_connection() {
    let Some(TestHub { hub, registry, mut events, .. }) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    const TASKS: u64 = 16;
    const ITERATIONS: usize = 25;
    let user = Uuid::new_v4();

    // Many sockets of the same user race: each connects, then disconnects itself, gets force-dropped,
    // disconnects an older socket of its own, or just waits to be replaced.
    let tasks: Vec<JoinHandle<_>> = (0..TASKS)
        .map(|seed| {
            let hub = hub.clone();
            tokio::spawn(async move {
                let mut rng = Rng(0xC0FFEE + seed);
                let mut connections = vec![];
                let (mut disconnected, mut forced) = (HashSet::new(), HashSet::new());
                for _ in 0..ITERATIONS {
                    let (connection_id, rx) = connect(&hub, user, 8);
                    connections.push((connection_id, rx));
                    match rng.below(4) {
                        0 => {
                            hub.request_disconnection(user, connection_id).unwrap();
                            disconnected.insert(connection_id);
                        }
                        1 => {
                            hub.request_drop(user, connection_id, DisconnectReason::SessionInvalid)
                                .unwrap();
                            forced.insert(connection_id);
                        }
                        2 => {
                            let (older, _) = &connections[rng.below(connections.len() as u64) as usize];
                            hub.request_disconnection(user, *older).unwrap();
                            disconnected.insert(*older);
                        }
                        _ => {}
                    }
                    if rng.below(2) == 0 {
                        tokio::task::yield_now().await;
                    }
                }
                (connections, disconnected, forced)
            })
        })
        .collect();
    let mut connections = vec![];
    let (mut disconnected, mut forced) = (HashSet::new(), HashSet::new());
    for task in tasks {
        let (task_connections, task_disconnected, task_forced) = task.await.unwrap();
        connections.extend(task_connections);
        disconnected.extend(task_disconnected);
        forced.extend(task_forced);
    }
    let seen = barrier(&hub, &mut events).await;

    let active = replay(&seen);
    let connects = seen
        .iter()
        .filter(|event| matches!(event, HubEvent::UserConnected { .. }))
        .count();
    assert_eq!(connects, connections.len(), "every connect is processed exactly once");

    let mut notices = BTreeMap::<DisconnectReason, u64>::new();
    let mut closed = 0;
    for (connection_id, rx) in &mut connections {
        let state = egress_state(rx);
        assert!(state.payloads.is_empty());
        assert_eq!(
            state.closed,
            active.get(&user) != Some(connection_id),
            "only the active connection stays open: {connection_id}"
        );
        if state.closed {
            closed += 1;
        }
        match state.notices.as_slice() {
            [] if state.closed => assert!(
                disconnected.contains(connection_id),
                "{connection_id} closed without a notice, but no disconnect was requested for it"
            ),
            [] | [DisconnectReason::Replaced] => {}
            [DisconnectReason::SessionInvalid] => {
                assert!(forced.contains(connection_id), "{connection_id} dropped unasked")
            }
            other => panic!("{connection_id} got unexpected close notices: {other:?}"),
        }
        for notice in state.notices {
            *notices.entry(notice).or_default() += 1;
        }
    }
    let disconnects = seen
        .iter()
        .filter(|event| matches!(event, HubEvent::UserDisconnected { .. }))
        .count();
    assert_eq!(closed, disconnects, "a channel is released for every disconnect");
    // No socket went away, so every forced drop delivered its notice.
    assert_eq!(drops(&seen), notices);
    assert_consistent(&hub, &registry, [user], &active).await;

    if let Some(connection_id) = active.get(&user) {
        hub.request_disconnection(user, *connection_id).unwrap();
    }
    barrier(&hub, &mut events).await;
    assert_eq!(hub.stats().await.connections, 0);
}

enum Client {
    Reader(JoinHandle<EgressState>),
    Slow(Egress),
}

#[test(serial = "hub-service")]
async fn churn_under_traffic_stays_consistent() {
    let Some(TestHub { hub, registry, mut events, .. }) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    const USERS: usize = 12;
    const TASKS_PER_USER: u64 = 2;
    const ITERATIONS: usize = 20;
    let users: Vec<Uuid> = (0..USERS).map(|_| Uuid::new_v4()).collect();

    // A slow client that never reads: the broadcast traffic must get it dropped as a slow consumer.
    let slow_user = Uuid::new_v4();
    let (slow_connection, mut slow_rx) = connect(&hub, slow_user, 2);
    let mut seen = barrier(&hub, &mut events).await;

    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let (known_tx, mut known_rx) = mpsc::unbounded_channel::<Uuid>();

    // Ordered broadcast traffic, and directed sends to connections that may already be gone.
    let broadcaster = {
        let hub = hub.clone();
        let stop = stop_rx.clone();
        tokio::spawn(async move {
            let mut seq = 0u64;
            while !*stop.borrow() {
                hub.broadcast(payload(format!("b-{seq:08}"))).await.unwrap();
                seq += 1;
                tokio::task::yield_now().await;
            }
        })
    };
    let direct_sender = {
        let hub = hub.clone();
        let stop = stop_rx.clone();
        tokio::spawn(async move {
            let mut known = vec![];
            let mut rng = Rng(0xD1EC7);
            while !*stop.borrow() {
                while let Ok(connection_id) = known_rx.try_recv() {
                    known.push(connection_id);
                }
                if !known.is_empty() {
                    let target = known[rng.below(known.len() as u64) as usize];
                    hub.send_to_connection(target, payload("d")).await.unwrap();
                }
                tokio::task::yield_now().await;
            }
        })
    };

    // Several sockets per user race, with drained clients on and off the Hub topic and slow ones.
    let tasks: Vec<JoinHandle<_>> = users
        .iter()
        .enumerate()
        .flat_map(|(idx, &user)| (0..TASKS_PER_USER).map(move |task| (idx as u64 * TASKS_PER_USER + task, user)))
        .map(|(seed, user)| {
            let hub = hub.clone();
            let known_tx = known_tx.clone();
            tokio::spawn(async move {
                let mut rng = Rng(0xBEEF + seed);
                let mut clients = vec![];
                for _ in 0..ITERATIONS {
                    let (connection_id, client) = match rng.below(3) {
                        0 => {
                            let (connection_id, rx) = connect(&hub, user, 2);
                            (connection_id, Client::Slow(rx))
                        }
                        1 => {
                            let (connection_id, rx) = connect_on(&hub, user, 64, vec![TopicKey::Stream, TopicKey::Hub]);
                            (connection_id, Client::Reader(spawn_reader(connection_id, rx)))
                        }
                        _ => {
                            let (connection_id, rx) = connect(&hub, user, 64);
                            (connection_id, Client::Reader(spawn_reader(connection_id, rx)))
                        }
                    };
                    let _ = known_tx.send(connection_id);
                    match rng.below(3) {
                        0 => hub.request_disconnection(user, connection_id).unwrap(),
                        1 => hub
                            .request_drop(user, connection_id, DisconnectReason::SessionInvalid)
                            .unwrap(),
                        _ => {}
                    }
                    clients.push((connection_id, client));
                    tokio::time::sleep(Duration::from_millis(rng.below(3))).await;
                }
                clients
            })
        })
        .collect();
    let mut clients = vec![];
    for task in tasks {
        clients.extend(task.await.unwrap());
    }
    let _ = stop_tx.send(true);
    broadcaster.await.unwrap();
    direct_sender.await.unwrap();

    seen.extend(barrier(&hub, &mut events).await);
    let active = replay(&seen);
    assert!(
        !active.contains_key(&slow_user),
        "the slow client must have been dropped"
    );
    assert_consistent(&hub, &registry, users.iter().copied(), &active).await;
    assert_eq!(
        egress_state(&mut slow_rx),
        EgressState {
            payloads: vec!["b-00000000".into(), "b-00000001".into()],
            notices: vec![],
            closed: true,
        },
        "the slow client {slow_connection} keeps its buffered messages"
    );

    // Release the survivors so every channel ends, then check what each client saw.
    for (user_id, connection_id) in &active {
        hub.request_disconnection(*user_id, *connection_id).unwrap();
    }
    seen.extend(barrier(&hub, &mut events).await);
    assert!(replay(&seen).is_empty());
    assert_consistent(&hub, &registry, users.iter().copied(), &HashMap::new()).await;

    let connects = seen
        .iter()
        .filter(|event| matches!(event, HubEvent::UserConnected { .. }))
        .count();
    assert_eq!(connects, clients.len() + 1, "every connect is processed exactly once");

    let mut notices = BTreeMap::<DisconnectReason, u64>::new();
    for (connection_id, client) in clients {
        let state = match client {
            Client::Reader(reader) => timeout(BARRIER_TIMEOUT, reader)
                .await
                .unwrap_or_else(|_| panic!("the channel of {connection_id} was never released"))
                .unwrap(),
            Client::Slow(mut rx) => egress_state(&mut rx),
        };
        assert!(state.closed, "the channel of {connection_id} was never released");
        assert!(
            state.notices.len() <= 1,
            "{connection_id} got several notices: {:?}",
            state.notices
        );
        let broadcasts: Vec<_> = state.payloads.iter().filter(|id| id.starts_with("b-")).collect();
        assert!(
            broadcasts.windows(2).all(|pair| pair[0] < pair[1]),
            "{connection_id} got broadcasts out of order"
        );
        for notice in state.notices {
            *notices.entry(notice).or_default() += 1;
        }
    }

    let dropped = drops(&seen);
    // A close notice is sent for every forced drop, but is lost when the channel is full.
    for (reason, count) in &notices {
        assert!(
            dropped.get(reason).copied().unwrap_or(0) >= *count,
            "more {reason} notices than drops: {notices:?} vs {dropped:?}"
        );
    }
    // Every client kept its channel until the hub released it, so none is mistaken for a dead one.
    assert_eq!(dropped.get(&DisconnectReason::DeadChannel), None, "{dropped:?}");
    assert!(dropped.get(&DisconnectReason::SlowConsumer).copied().unwrap_or(0) >= 1);
    assert_eq!(hub.stats().await.connections, 0);
}

// --- Two instances ------------------------------------------------------------------------------

#[test(serial = "hub-service")]
async fn a_connect_on_another_instance_takes_the_user_over() {
    let (Some(mut a), Some(mut b)) = (
        create_test_hub(TestHubOptions::default()).await,
        create_test_hub(TestHubOptions::default()).await,
    ) else {
        return;
    };
    let user = Uuid::new_v4();

    let (on_a, mut a_rx) = connect(&a.hub, user, 8);
    barrier(&a.hub, &mut a.events).await;
    let (on_b, mut b_rx) = connect(&b.hub, user, 8);
    let seen_b = barrier(&b.hub, &mut b.events).await;

    // A learns about B's connect from the registry change notice, which is not ordered with A's commands.
    let seen = wait_event(
        &mut a.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id, .. } if *connection_id == on_a),
    )
    .await
    .expect("the instance that lost the user must drop its connection");
    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(on_a)]);
    let seen = [seen, barrier(&a.hub, &mut a.events).await].concat();

    assert_eq!(
        egress_state(&mut a_rx),
        closed_with(Some(DisconnectReason::RegistryInactive))
    );
    assert_eq!(egress_state(&mut b_rx), open());
    assert_eq!(drops(&seen), dropped(&[(DisconnectReason::RegistryInactive, 1)]));
    assert_eq!(drops(&seen_b), dropped(&[]));
    assert_consistent(&a.hub, &a.registry, [], &HashMap::new()).await;
    assert_consistent(&b.hub, &b.registry, [user], &HashMap::from([(user, on_b)])).await;

    // B's disconnect removes the row; A has nothing left to drop.
    b.hub.request_disconnection(user, on_b).unwrap();
    barrier(&b.hub, &mut b.events).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(lifecycle_of(&barrier(&a.hub, &mut a.events).await, user), []);
    assert_eq!(find_registry_connection(&a.registry, user).await, None);
}

#[test(serial = "hub-service")]
async fn users_bouncing_between_instances_converge_to_the_registry() {
    let (Some(mut a), Some(mut b)) = (
        create_test_hub(TestHubOptions::default()).await,
        create_test_hub(TestHubOptions::default()).await,
    ) else {
        return;
    };
    const USERS: u64 = 16;
    const ITERATIONS: usize = 20;
    let users: Vec<Uuid> = (0..USERS).map(|_| Uuid::new_v4()).collect();

    // Each user reconnects over and over through a random instance, sometimes disconnecting itself.
    let tasks: Vec<JoinHandle<_>> = users
        .iter()
        .enumerate()
        .map(|(seed, &user)| {
            let hubs = [a.hub.clone(), b.hub.clone()];
            tokio::spawn(async move {
                let mut rng = Rng(0xB0B0 + seed as u64);
                let mut connections = vec![];
                let mut disconnected = HashSet::new();
                for _ in 0..ITERATIONS {
                    let instance = rng.below(2) as usize;
                    let (connection_id, rx) = connect(&hubs[instance], user, 16);
                    if rng.below(3) == 0 {
                        hubs[instance].request_disconnection(user, connection_id).unwrap();
                        disconnected.insert(connection_id);
                    }
                    connections.push((instance, connection_id, rx));
                    tokio::time::sleep(Duration::from_millis(rng.below(3))).await;
                }
                (connections, disconnected)
            })
        })
        .collect();
    let mut connections = vec![];
    let mut disconnected = HashSet::new();
    for task in tasks {
        let (task_connections, task_disconnected) = task.await.unwrap();
        connections.extend(task_connections);
        disconnected.extend(task_disconnected);
    }

    // The instances converge through the registry change notices: in the end each user has at most one
    // connection over both instances, the one the registry holds.
    let (mut seen_a, mut seen_b) = (vec![], vec![]);
    let deadline = tokio::time::Instant::now() + BARRIER_TIMEOUT;
    let (active_a, active_b) = loop {
        seen_a.extend(barrier(&a.hub, &mut a.events).await);
        seen_b.extend(barrier(&b.hub, &mut b.events).await);
        let (active_a, active_b) = (replay(&seen_a), replay(&seen_b));
        let mut converged = true;
        for user in &users {
            let local: Vec<Uuid> = [active_a.get(user), active_b.get(user)]
                .into_iter()
                .flatten()
                .copied()
                .collect();
            let held: Vec<Uuid> = find_registry_connection(&a.registry, *user).await.into_iter().collect();
            converged &= local == held;
        }
        if converged {
            break (active_a, active_b);
        }
        assert!(tokio::time::Instant::now() < deadline, "the instances did not converge");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(
        count_connects(&seen_a) + count_connects(&seen_b),
        connections.len(),
        "every connect is processed exactly once, on its own instance"
    );
    assert_eq!(a.hub.stats().await.connections, active_a.len());
    assert_eq!(b.hub.stats().await.connections, active_b.len());

    let mut notices = BTreeMap::<DisconnectReason, u64>::new();
    for (instance, connection_id, rx) in &mut connections {
        let active = [&active_a, &active_b][*instance].values().any(|id| id == connection_id);
        let state = egress_state(rx);
        assert_eq!(
            state.closed, !active,
            "only the converged connections stay open: {connection_id}"
        );
        match state.notices.as_slice() {
            [] if state.closed => assert!(
                disconnected.contains(connection_id),
                "{connection_id} closed without a notice, but no disconnect was requested for it"
            ),
            [] | [DisconnectReason::Replaced] | [DisconnectReason::RegistryInactive] => {}
            other => panic!("{connection_id} got unexpected close notices: {other:?}"),
        }
        for notice in state.notices {
            *notices.entry(notice).or_default() += 1;
        }
    }
    // No socket went away, so every drop delivered its notice.
    let mut dropped = drops(&seen_a);
    for (reason, count) in drops(&seen_b) {
        *dropped.entry(reason).or_default() += count;
    }
    assert_eq!(dropped, notices);

    for (test, active) in [(&mut a, &active_a), (&mut b, &active_b)] {
        for (user, connection_id) in active {
            test.hub.request_disconnection(*user, *connection_id).unwrap();
        }
        barrier(&test.hub, &mut test.events).await;
    }
}

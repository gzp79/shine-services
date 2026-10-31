use shine_game::replica::{
    decode_snapshot, encode_snapshot, stable_hash_of, CommandEnvelope, CommandHeader, PixelMapCommand, PixelMapState,
    ReplayState, Replica, PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH,
};
use shine_test::test;
use uuid::Uuid;

fn paint(x: u8, y: u8, color: u8) -> PixelMapCommand {
    PixelMapCommand::Paint { x, y, color }
}

fn apply(state: &mut PixelMapState, command: &PixelMapCommand) {
    let header = CommandHeader {
        version: 1,
        command_id: Uuid::nil(),
        user_id: Uuid::nil(),
        chain_hash: 0,
    };
    state.apply(&header, command);
}

fn pixel(state: &PixelMapState, x: usize, y: usize) -> u8 {
    state.pixels[y * PIXEL_MAP_WIDTH + x]
}

#[test]
fn a_new_map_is_empty_and_of_the_fixed_size() {
    let state = PixelMapState::default();

    assert_eq!(state.pixels.len(), PIXEL_MAP_WIDTH * PIXEL_MAP_HEIGHT);
    assert!(state.pixels.iter().all(|&color| color == 0));
}

#[test]
fn painting_sets_the_pixel_and_painting_again_replaces_it() {
    let mut state = PixelMapState::default();

    apply(&mut state, &paint(0, 0, 3));
    apply(
        &mut state,
        &paint(PIXEL_MAP_WIDTH as u8 - 1, PIXEL_MAP_HEIGHT as u8 - 1, 15),
    );
    apply(&mut state, &paint(0, 0, 7));

    assert_eq!(pixel(&state, 0, 0), 7);
    assert_eq!(pixel(&state, PIXEL_MAP_WIDTH - 1, PIXEL_MAP_HEIGHT - 1), 15);
    assert_eq!(state.pixels.iter().filter(|&&color| color != 0).count(), 2);
}

#[test]
fn a_paint_outside_the_map_or_the_palette_is_ignored_and_is_not_well_formed() {
    let outside = [
        paint(PIXEL_MAP_WIDTH as u8, 0, 1),
        paint(0, PIXEL_MAP_HEIGHT as u8, 1),
        paint(255, 255, 1),
        paint(1, 1, PIXEL_MAP_COLORS),
    ];
    for command in &outside {
        let mut state = PixelMapState::default();

        apply(&mut state, command);

        assert_eq!(state, PixelMapState::default(), "{command:?}");
        assert!(!PixelMapState::is_well_formed(command), "{command:?}");
    }
    assert!(PixelMapState::is_well_formed(&paint(31, 31, PIXEL_MAP_COLORS - 1)));
    assert!(PixelMapState::is_well_formed(&paint(0, 0, 0)), "color 0 erases");
}

#[test]
fn a_state_of_the_wrong_size_is_not_panicked_on() {
    // What a corrupt snapshot could decode to.
    let mut state = PixelMapState { pixels: vec![0; 3] };

    apply(&mut state, &paint(10, 10, 1));
    apply(&mut state, &paint(0, 0, 1));

    assert_eq!(state.pixels, vec![1, 0, 0]);
}

#[test]
fn painting_different_pixels_gives_the_same_state_in_any_order() {
    let commands = [paint(1, 2, 3), paint(4, 5, 6), paint(7, 8, 9)];
    let (mut forward, mut backward) = (PixelMapState::default(), PixelMapState::default());

    for command in &commands {
        apply(&mut forward, command);
    }
    for command in commands.iter().rev() {
        apply(&mut backward, command);
    }

    assert_eq!(forward, backward);
    assert_eq!(stable_hash_of(&forward), stable_hash_of(&backward));
    assert_ne!(stable_hash_of(&forward), stable_hash_of(&PixelMapState::default()));
}

#[test]
fn painting_the_same_pixel_in_another_order_gives_another_state() {
    let (mut first, mut second) = (PixelMapState::default(), PixelMapState::default());

    apply(&mut first, &paint(1, 1, 2));
    apply(&mut first, &paint(1, 1, 3));
    apply(&mut second, &paint(1, 1, 3));
    apply(&mut second, &paint(1, 1, 2));

    assert_ne!(stable_hash_of(&first), stable_hash_of(&second));
}

#[test]
fn replicas_replaying_the_same_log_agree_and_a_snapshot_resumes_to_the_same_state() {
    let commands = [
        paint(0, 0, 1),
        paint(5, 5, 2),
        paint(5, 5, 3),
        paint(31, 0, 4),
        paint(40, 0, 5),
    ];
    let mut chain_hash = 0;
    let mut envelopes = Vec::new();
    for (i, command) in commands.iter().enumerate() {
        let envelope = CommandEnvelope::new(
            i as i64 + 1,
            Uuid::from_u128(i as u128),
            Uuid::nil(),
            chain_hash,
            command.clone(),
        );
        chain_hash = envelope.header.chain_hash;
        envelopes.push(envelope);
    }
    let (mut full, mut prefix) = (Replica::<PixelMapState>::new(), Replica::<PixelMapState>::new());
    for (i, envelope) in envelopes.iter().enumerate() {
        full.apply_next(envelope).unwrap();
        if i < 3 {
            prefix.apply_next(envelope).unwrap();
        }
    }

    let bytes = encode_snapshot(prefix.state()).unwrap();
    let mut resumed = Replica::from_snapshot(
        3,
        decode_snapshot::<PixelMapState>(&bytes).unwrap(),
        prefix.chain_hash(),
    );
    for envelope in &envelopes[3..] {
        resumed.apply_next(envelope).unwrap();
    }

    assert_eq!(resumed.state(), full.state());
    assert_eq!(resumed.hash(), full.hash());
    assert_eq!(pixel(full.state(), 5, 5), 3);
    assert_eq!(pixel(full.state(), 31, 0), 4);
    assert_eq!(
        full.state().pixels.iter().filter(|&&color| color != 0).count(),
        3,
        "the paint outside is ignored"
    );
}

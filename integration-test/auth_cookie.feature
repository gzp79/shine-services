Feature: Check auth cookie validation

  Background:
    * url karate.properties['identityUrl']

  # expired tid -> tid is removed
  # invalid signature sid -> sid is removed
  # invalid signature tid -> tid is removed
  # invalid signature eid -> eid is removed

  # todo: create sid,eid,tid for 4 users
  # +: user1.(sid,tid,eid)
  # !: user2.sid, user3.tid, user4.eid
  # create ep that only validates the cookies without any login

  # -: missing
  # !: not matching (ex different user id). When multiple ! are present in a row it's assumed all of them are different
  # +: ok, valid cookie
  # e: expired to perform a full test, but skipped for now

  # tid | sid | eid | action
  #  -  |  -  |  -  |  noop
  #  -  |  -  |  +  |  remove eid
  #  -  |  -  |  !  |  remove eid
  #  -  |  !  |  -  |  noop
  #  -  |  !  |  +  |  remove eid
  #  -  |  !  |  !  |  remove eid
  #  -  |  +  |  -  |  noop
  #  -  |  +  |  +  |  noop
  #  -  |  +  |  !  |  remove eid

  #  !  |  -  |  -  |  noop
  #  !  |  -  |  +  |  remove eid
  #  !  |  -  |  !  |  remove eid
  #  !  |  !  |  -  |  remove sid
  #  !  |  !  |  +  |  remove sid, eid
  #  !  |  !  |  !  |  remove sid, eid
  #  !  |  +  |  -  |  remove sid
  #  !  |  +  |  +  |  remove sid, eid
  #  !  |  +  |  !  |  remove sid, eid

  #  +  |  -  |  -  |  noop
  #  +  |  -  |  +  |  remove eid
  #  +  |  -  |  !  |  remove eid
  #  +  |  !  |  -  |  remove sid
  #  +  |  !  |  +  |  remove sid, eid
  #  +  |  !  |  !  |  remove sid, eid
  #  +  |  +  |  -  |  noop
  #  +  |  +  |  +  |  noop
  #  +  |  +  |  !  |  remove eid

Scenario:

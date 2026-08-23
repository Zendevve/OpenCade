use openfight_protocol::RoomState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomEvent {
    Accept,
    Start,
    Finish,
    Cancel,
    Decline,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("cannot apply {event:?} while room is {state:?}")]
pub struct InvalidTransition {
    pub state: RoomState,
    pub event: RoomEvent,
}

pub fn transition(state: RoomState, event: RoomEvent) -> Result<RoomState, InvalidTransition> {
    match (&state, event) {
        (RoomState::Waiting | RoomState::Challenging, RoomEvent::Accept) => {
            Ok(RoomState::Connecting)
        }
        (RoomState::Connecting, RoomEvent::Start) => Ok(RoomState::Playing),
        (RoomState::Playing, RoomEvent::Finish) => Ok(RoomState::Finished),
        (
            RoomState::Waiting | RoomState::Ready | RoomState::Challenging | RoomState::Connecting,
            RoomEvent::Cancel | RoomEvent::Decline,
        ) => Ok(RoomState::Cancelled),
        _ => Err(InvalidTransition { state, event }),
    }
}

pub fn from_database(value: &str) -> Result<RoomState, String> {
    match value {
        "WAITING" => Ok(RoomState::Waiting),
        "READY" => Ok(RoomState::Ready),
        "CHALLENGING" => Ok(RoomState::Challenging),
        "CONNECTING" => Ok(RoomState::Connecting),
        "PLAYING" => Ok(RoomState::Playing),
        "FINISHED" => Ok(RoomState::Finished),
        "CANCELLED" => Ok(RoomState::Cancelled),
        other => Err(format!("unknown room state: {other}")),
    }
}

pub fn to_database(state: &RoomState) -> &'static str {
    match state {
        RoomState::Waiting => "WAITING",
        RoomState::Ready => "READY",
        RoomState::Challenging => "CHALLENGING",
        RoomState::Connecting => "CONNECTING",
        RoomState::Playing => "PLAYING",
        RoomState::Finished => "FINISHED",
        RoomState::Cancelled => "CANCELLED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_the_match_happy_path() {
        let connecting = transition(RoomState::Waiting, RoomEvent::Accept).expect("accept");
        let playing = transition(connecting, RoomEvent::Start).expect("start");
        let finished = transition(playing, RoomEvent::Finish).expect("finish");
        assert_eq!(finished, RoomState::Finished);
    }

    #[test]
    fn terminal_states_reject_all_transitions() {
        for state in [RoomState::Finished, RoomState::Cancelled] {
            for event in [
                RoomEvent::Accept,
                RoomEvent::Start,
                RoomEvent::Finish,
                RoomEvent::Cancel,
                RoomEvent::Decline,
            ] {
                assert!(transition(state.clone(), event).is_err());
            }
        }
    }

    #[test]
    fn database_mapping_round_trips_every_state() {
        for state in [
            RoomState::Waiting,
            RoomState::Ready,
            RoomState::Challenging,
            RoomState::Connecting,
            RoomState::Playing,
            RoomState::Finished,
            RoomState::Cancelled,
        ] {
            assert_eq!(from_database(to_database(&state)), Ok(state));
        }
    }
}

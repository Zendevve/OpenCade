import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Challenge } from "../lib/api";
import type { RoomPayload } from "@opencade/protocol";
import { trackProductEvent } from "../lib/telemetry";

type Props = {
  token: string;
  userId: string;
  gameId: string;
  onBack: () => void;
  onMatch: (roomId: string) => void;
};

export default function Lobby({ token, userId, gameId, onBack, onMatch }: Props) {
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState("Creating your lobby presence…");
  const [lobbyRoom, setLobbyRoom] = useState<RoomPayload | null>(null);
  const [inviteCode, setInviteCode] = useState("");
  const [joinCode, setJoinCode] = useState("");
  const [responsePending, setResponsePending] = useState<string | null>(null);
  const [responseError, setResponseError] = useState("");
  const lobbyTracked = useRef(false);
  useEffect(() => {
    let active = true;
    api
      .joinLobby(token, gameId)
      .then((room) => {
        if (active) {
          setLobbyRoom(room);
          setNotice("Ready for challenges");
          if (!lobbyTracked.current) {
            lobbyTracked.current = true;
            void trackProductEvent(token, "lobby_entered", gameId).catch(() => undefined);
          }
        }
      })
      .catch((error: Error) => {
        if (active) setNotice(error.message);
      });
    return () => {
      active = false;
    };
  }, [token, gameId]);
  const lobby = useQuery({
    queryKey: ["lobby", gameId],
    queryFn: () => api.lobby(token, gameId),
    refetchInterval: 3_000,
  });
  const incoming = useQuery({
    queryKey: ["challenges"],
    queryFn: () => api.incomingChallenges(token),
    refetchInterval: 3_000,
  });
  const challenge = useMutation({
    mutationFn: (challengedId: string) => api.challenge(token, gameId, challengedId),
    onSuccess: () => setNotice("Challenge sent. Waiting for response…"),
    onError: (error) => setNotice(error.message),
  });
  const createInvite = useMutation({
    mutationFn: () => {
      if (!lobbyRoom) throw new Error("Lobby room is not ready");
      return api.createInvite(token, lobbyRoom.id);
    },
    onSuccess: (invite) => {
      setInviteCode(invite.code);
      setNotice(`Invite expires ${new Date(invite.expires_at).toLocaleTimeString()}`);
    },
    onError: (error) => setNotice(error.message),
  });
  const joinInvite = useMutation({
    mutationFn: () => api.joinInvite(token, joinCode),
    onSuccess: (room) => onMatch(room.id),
    onError: (error) => setNotice(error.message),
  });
  const pending = incoming.data?.challenges.filter((item) => item.game_id === gameId) ?? [];
  const peers = lobby.data?.members.filter((member) => member.user_id !== userId) ?? [];
  const respond = async (item: Challenge, accept: boolean) => {
    setResponsePending(item.id);
    setResponseError("");
    try {
      const result = accept
        ? await api.acceptChallenge(token, item.id)
        : await api.declineChallenge(token, item.id);
      await queryClient.invalidateQueries({ queryKey: ["challenges"] });
      if (accept) onMatch(result.room_id);
    } catch (error) {
      setResponseError(error instanceof Error ? error.message : "Challenge response failed");
    } finally {
      setResponsePending(null);
    }
  };
  return (
    <section>
      <button className="back" onClick={onBack}>
        ← Games
      </button>
      <div className="section-heading">
        <div>
          <p className="eyebrow">Lobby · {gameId}</p>
          <h2>Choose an opponent</h2>
        </div>
        <span className="count">{notice}</span>
      </div>
      <article className="challenge-banner">
        <div>
          <strong>Private alpha invite</strong>
          <span>{inviteCode || "Create a one-use 15-minute code or join one."}</span>
        </div>
        <div>
          <input
            aria-label="Invite code"
            maxLength={10}
            placeholder="A1B2C3D4E5"
            value={joinCode}
            onChange={(event) => setJoinCode(event.target.value.toUpperCase())}
          />
          <button
            className="secondary"
            disabled={!lobbyRoom || createInvite.isPending}
            onClick={() => createInvite.mutate()}
          >
            Create code
          </button>
          <button
            className="primary compact"
            disabled={joinCode.length !== 10 || joinInvite.isPending}
            onClick={() => joinInvite.mutate()}
          >
            Join code
          </button>
        </div>
      </article>
      {(lobby.isError || incoming.isError || responseError) && (
        <div className="form-error" role="alert">
          {responseError || lobby.error?.message || incoming.error?.message}
          <button
            className="secondary compact"
            onClick={() => {
              void lobby.refetch();
              void incoming.refetch();
            }}
          >
            Retry lobby
          </button>
        </div>
      )}
      {pending.map((item) => (
        <article className="challenge-banner" key={item.id}>
          <div>
            <strong>Incoming challenge</strong>
            <span>Room {item.room_id.slice(0, 8)}</span>
          </div>
          <div>
            <button
              className="secondary"
              disabled={responsePending === item.id}
              onClick={() => void respond(item, false)}
            >
              Decline
            </button>
            <button
              className="primary compact"
              disabled={responsePending === item.id}
              onClick={() => void respond(item, true)}
            >
              Accept
            </button>
          </div>
        </article>
      ))}
      <div className="player-list">
        {peers.map((peer) => (
          <article className="player-row" key={peer.user_id}>
            <div className="avatar">{peer.username.slice(0, 2).toUpperCase()}</div>
            <div>
              <strong>{peer.username}</strong>
              <small>{peer.rtt_ms === null ? "Latency pending" : `${peer.rtt_ms} ms`}</small>
            </div>
            <button
              className="primary compact"
              disabled={challenge.isPending}
              onClick={() => challenge.mutate(peer.user_id)}
            >
              Challenge
            </button>
          </article>
        ))}
        {!lobby.isPending && !lobby.isError && peers.length === 0 && (
          <div className="empty">
            <strong>No opponents yet</strong>
            <span>Keep this lobby open while another player joins {gameId}.</span>
          </div>
        )}
      </div>
    </section>
  );
}

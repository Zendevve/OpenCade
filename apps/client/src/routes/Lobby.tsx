import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Challenge } from "../lib/api";

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
  useEffect(() => {
    let active = true;
    api
      .joinLobby(token, gameId)
      .then(() => {
        if (active) setNotice("Ready for challenges");
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
  const pending = incoming.data?.challenges.filter((item) => item.game_id === gameId) ?? [];
  const peers = lobby.data?.members.filter((member) => member.user_id !== userId) ?? [];
  const respond = async (item: Challenge, accept: boolean) => {
    const result = accept
      ? await api.acceptChallenge(token, item.id)
      : await api.declineChallenge(token, item.id);
    await queryClient.invalidateQueries({ queryKey: ["challenges"] });
    if (accept) onMatch(result.room_id);
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
      {pending.map((item) => (
        <article className="challenge-banner" key={item.id}>
          <div>
            <strong>Incoming challenge</strong>
            <span>Room {item.room_id.slice(0, 8)}</span>
          </div>
          <div>
            <button className="secondary" onClick={() => void respond(item, false)}>
              Decline
            </button>
            <button className="primary compact" onClick={() => void respond(item, true)}>
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
        {!lobby.isPending && peers.length === 0 && (
          <div className="empty">
            <strong>No opponents yet</strong>
            <span>Keep this lobby open while another player joins {gameId}.</span>
          </div>
        )}
      </div>
    </section>
  );
}

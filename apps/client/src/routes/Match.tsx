import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import { downloadMatchReport } from "../lib/report";

export default function Match({
  token,
  roomId,
  onDone,
}: {
  token: string;
  roomId: string;
  onDone: () => void;
}) {
  const room = useQuery({
    queryKey: ["room", roomId],
    queryFn: () => api.room(token, roomId),
    refetchInterval: 2_000,
  });
  const state = room.data?.state ?? "connecting";
  const steps = ["connecting", "playing", "finished"];
  const active = Math.max(0, steps.indexOf(state));
  const heading =
    state === "connecting"
      ? "Establishing peer session"
      : state === "playing"
        ? "Match in progress"
        : state === "finished"
          ? "Match complete"
          : `Room ${state}`;
  return (
    <section className="match-stage">
      <p className="eyebrow">Room {roomId.slice(0, 8)}</p>
      <h2>{heading}</h2>
      <div className="match-orbit">
        <span>YOU</span>
        <i />
        <span>PEER</span>
      </div>
      <ol className="match-steps">
        {steps.map((step, index) => (
          <li className={index <= active ? "active" : ""} key={step}>
            {step}
          </li>
        ))}
      </ol>
      {room.isError && <p className="form-error">{room.error.message}</p>}
      <div className="match-actions">
        {room.data && (
          <button className="secondary" onClick={() => downloadMatchReport(room.data)}>
            Export report
          </button>
        )}
        <button className="secondary" onClick={onDone}>
          Return to games
        </button>
      </div>
    </section>
  );
}

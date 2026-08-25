import { useQueries, useQuery } from "@tanstack/react-query";
import { api, type Game } from "../lib/api";
import { scanGame } from "../lib/native";
import CampaignDashboard from "../components/CampaignDashboard";

type Props = { token: string; onSelect: (gameId: string) => void };

export default function Games({ token, onSelect }: Props) {
  const games = useQuery({ queryKey: ["games"], queryFn: () => api.games(token) });
  if (games.isPending) return <StatusCard title="Loading games" detail="Reading server catalog…" />;
  if (games.isError) return <StatusCard title="Games unavailable" detail={games.error.message} />;
  return (
    <>
      <CampaignDashboard token={token} />
      <GameCatalog games={games.data.games} onSelect={onSelect} />
    </>
  );
}

function GameCatalog({ games, onSelect }: { games: Game[]; onSelect: (gameId: string) => void }) {
  const availability = useQueries({
    queries: games.map((game) => ({
      queryKey: ["availability", game.id],
      queryFn: () => scanGame(game.id),
      staleTime: 30_000,
    })),
  });
  return (
    <section aria-labelledby="games-heading">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Game catalog</p>
          <h2 id="games-heading">Choose your arena</h2>
        </div>
        <span className="count">{games.length} in catalog</span>
      </div>
      <div className="game-grid">
        {games.map((game, index) => (
          <button className="game-card" key={game.id} onClick={() => onSelect(game.id)}>
            <span className="game-mark" aria-hidden="true">
              {game.name.slice(0, 2).toUpperCase()}
            </span>
            <span className="game-copy">
              <strong>{game.name}</strong>
              <small>
                {game.emulator} · {game.default_version ?? "version pending"} ·{" "}
                {availability[index]?.data?.available ? "installed" : "ROM not detected"}
              </small>
            </span>
            <span className="arrow" aria-hidden="true">
              →
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}

function StatusCard({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="status-card" role="status">
      <strong>{title}</strong>
      <span>{detail}</span>
    </div>
  );
}

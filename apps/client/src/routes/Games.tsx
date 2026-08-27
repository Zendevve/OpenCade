import { useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { api, type Game } from "../lib/api";
import { scanGame } from "../lib/native";
import MatchReadiness from "../components/MatchReadiness";
import TelemetryConsent from "../components/TelemetryConsent";
import { trackProductEvent } from "../lib/telemetry";

type Props = { token: string; onSelect: (gameId: string) => void };

export default function Games({ token, onSelect }: Props) {
  const [selectedGame, setSelectedGame] = useState<Game | null>(null);
  const games = useQuery({ queryKey: ["games"], queryFn: () => api.games(token) });
  if (games.isPending) return <StatusCard title="Loading games" detail="Reading server catalog…" />;
  if (games.isError) return <StatusCard title="Games unavailable" detail={games.error.message} />;
  if (selectedGame) {
    return (
      <MatchReadiness
        token={token}
        game={selectedGame}
        onBack={() => setSelectedGame(null)}
        onContinue={() => onSelect(selectedGame.id)}
      />
    );
  }
  return (
    <>
      <TelemetryConsent />
      <GameCatalog
        games={games.data.games}
        onSelect={(game) => {
          setSelectedGame(game);
          void trackProductEvent(token, "game_selected", game.id).catch(() => undefined);
        }}
      />
    </>
  );
}

function GameCatalog({ games, onSelect }: { games: Game[]; onSelect: (game: Game) => void }) {
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
          <button className="game-card" key={game.id} onClick={() => onSelect(game)}>
            <span className="game-mark" aria-hidden="true">
              {game.name.slice(0, 2).toUpperCase()}
            </span>
            <span className="game-copy">
              <strong>{game.name}</strong>
              <small>
                {game.emulator} · {game.default_version ?? "version pending"} ·{" "}
                {availabilityLabel(availability[index])}
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

function availabilityLabel(
  query: { isPending: boolean; isError: boolean; data?: { available: boolean } } | undefined
): string {
  if (!query || query.isPending) return "checking local files…";
  if (query.isError) return "scan failed — select to retry";
  if (!query.data) return "scan unavailable";
  return query.data.available ? "installed" : "ROM not detected";
}

function StatusCard({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="status-card" role="status">
      <strong>{title}</strong>
      <span>{detail}</span>
    </div>
  );
}

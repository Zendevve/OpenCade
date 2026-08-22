import React from "react";

type Game = {
  id: string;
  name: string;
  emulator: string;
};

const PLACEHOLDER_GAMES: Game[] = [
  { id: "sfiii3", name: "Street Fighter III: 3rd Strike", emulator: "fbneo" },
  { id: "garou", name: "Garou: Mark of the Wolves", emulator: "fbneo" },
  { id: "kof98", name: "The King of Fighters '98", emulator: "fbneo" },
  { id: "mvc2", name: "Marvel vs. Capcom 2", emulator: "flycast" },
];

export default function Games() {
  const [filter, setFilter] = React.useState("");

  const filtered = PLACEHOLDER_GAMES.filter(
    (g) =>
      g.name.toLowerCase().includes(filter.toLowerCase()) ||
      g.id.toLowerCase().includes(filter.toLowerCase())
  );

  return (
    <div style={{ padding: 16 }}>
      <h1>Games</h1>
      <p>Placeholder list — wired to game-definitions and emulator-sdk scan later.</p>
      <input
        placeholder="Filter games..."
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        style={{ marginBottom: 12, padding: 6, width: 280 }}
      />
      <ul>
        {filtered.map((g) => (
          <li key={g.id}>
            <strong>{g.name}</strong> <code>{g.id}</code> — {g.emulator}
          </li>
        ))}
      </ul>
      {filtered.length === 0 && <p>No games match &ldquo;{filter}&rdquo;.</p>}
    </div>
  );
}

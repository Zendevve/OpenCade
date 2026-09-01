import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { Game } from "../lib/api";
import { isDesktopRuntime, retroarchPreflight, runNetworkTest } from "../lib/native";
import { assessMatchReadiness, type ReadinessState } from "../lib/readiness";
import { trackProductEvent } from "../lib/telemetry";

type Props = {
  token: string;
  game: Game;
  onBack: () => void;
  onContinue: () => void;
};

export default function MatchReadiness({ token, game, onBack, onContinue }: Props) {
  const desktop = isDesktopRuntime();
  const [notice, setNotice] = useState("");
  const preflight = useQuery({
    queryKey: ["match-readiness", "preflight", game.id],
    queryFn: () => retroarchPreflight(game.id),
    enabled: desktop,
    retry: false,
  });
  const network = useQuery({
    queryKey: ["match-readiness", "network"],
    queryFn: runNetworkTest,
    enabled: desktop,
    retry: false,
    staleTime: 30_000,
  });
  const assessment = assessMatchReadiness({
    desktop,
    controlPlaneReady: true,
    preflightStatus: queryStatus(desktop, preflight.status),
    preflight: preflight.data,
    networkStatus: queryStatus(desktop, network.status),
    network: network.data,
    gameId: game.id,
    isDev: import.meta.env.DEV,
  });
  const continueToLobby = () => {
    if (assessment.canContinue) {
      void trackProductEvent(token, "readiness_completed", game.id).catch(() => undefined);
      onContinue();
      return;
    }
    const blockedChecks = assessment.checks
      .filter((check) => check.required && check.state === "blocked")
      .map((check) => check.id);
    if (blockedChecks.length > 0) {
      void trackProductEvent(token, "readiness_blocked", game.id, blockedChecks).catch(
        () => undefined
      );
    }
    setNotice("Resolve the blocked requirements before entering the lobby.");
  };
  const retry = async () => {
    if (!desktop) {
      setNotice("Open the desktop client to run native readiness checks.");
      return;
    }
    setNotice("Running the readiness checks again…");
    await Promise.all([preflight.refetch(), network.refetch()]);
    setNotice("Readiness checks updated.");
  };

  return (
    <section className="readiness" aria-labelledby="readiness-heading">
      <button className="back" onClick={onBack}>
        ← Game catalog
      </button>
      <div className="readiness-heading">
        <div>
          <p className="eyebrow">Match readiness · {game.name}</p>
          <h2 id="readiness-heading">Prepare this machine</h2>
          <p className="readiness-intro">
            Complete the required checks before another player commits to the match.
          </p>
        </div>
        <strong className="readiness-progress">
          {assessment.readyRequired}/{assessment.requiredTotal} required checks ready
        </strong>
      </div>
      <ol className="readiness-list">
        {assessment.checks.map((check) => (
          <li className={`readiness-check ${check.state}`} key={check.id}>
            <span className="readiness-symbol" aria-hidden="true">
              {stateSymbol(check.state)}
            </span>
            <div className="readiness-copy">
              <h3>{check.title}</h3>
              <p>{check.detail}</p>
            </div>
            <span className="readiness-state">
              {stateLabel(check.state)}
              {!check.required && " · advisory"}
            </span>
          </li>
        ))}
      </ol>
      <p className="readiness-notice" aria-live="polite">
        {notice ||
          (assessment.canContinue
            ? "This machine is ready to enter the lobby."
            : "Blocked checks include a specific recovery action.")}
      </p>
      <div className="readiness-actions">
        <button className="secondary" onClick={() => void retry()}>
          Run checks again
        </button>
        <button className="primary" onClick={continueToLobby}>
          Enter lobby
        </button>
      </div>
    </section>
  );
}

function queryStatus(
  enabled: boolean,
  status: "pending" | "error" | "success"
): "pending" | "error" | "success" {
  return enabled ? status : "error";
}

function stateLabel(state: ReadinessState): string {
  switch (state) {
    case "ready":
      return "Ready";
    case "warning":
      return "Check advised";
    case "blocked":
      return "Action required";
    case "pending":
      return "Checking";
  }
}

function stateSymbol(state: ReadinessState): string {
  switch (state) {
    case "ready":
      return "✓";
    case "warning":
      return "!";
    case "blocked":
      return "×";
    case "pending":
      return "…";
  }
}

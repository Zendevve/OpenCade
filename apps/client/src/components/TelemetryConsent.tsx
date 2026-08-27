import { useState } from "react";
import { getTelemetryConsent, setTelemetryConsent } from "../lib/telemetry";

export default function TelemetryConsent() {
  const [consent, setConsent] = useState<boolean | null>(() => getTelemetryConsent());
  const choose = (next: boolean) => {
    if (setTelemetryConsent(next)) setConsent(next);
  };

  return (
    <section className="telemetry-consent" aria-labelledby="telemetry-heading">
      <div className="telemetry-copy">
        <p className="eyebrow">Privacy control</p>
        <h2 id="telemetry-heading">Anonymous product telemetry</h2>
        <p>
          Share game selection and readiness outcomes to help improve the match setup. OpenFight
          never sends usernames, ROM paths, network endpoints, or error text through telemetry.
        </p>
      </div>
      <div className="telemetry-choice" aria-label="Anonymous product telemetry setting">
        <button
          className={consent === true ? "primary compact" : "secondary"}
          aria-pressed={consent === true}
          onClick={() => choose(true)}
        >
          Share outcomes
        </button>
        <button
          className={consent === false ? "primary compact" : "secondary"}
          aria-pressed={consent === false}
          onClick={() => choose(false)}
        >
          Don’t share
        </button>
      </div>
      <span className="telemetry-state" role="status">
        {consent === null
          ? "No choice saved · sharing is off"
          : consent
            ? "Sharing on"
            : "Sharing off"}
      </span>
    </section>
  );
}

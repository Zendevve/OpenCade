import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";

export default function CampaignDashboard({ token }: { token: string }) {
  const campaign = useQuery({
    queryKey: ["alpha-campaign"],
    queryFn: () => api.campaign(token),
    staleTime: 15_000,
  });
  if (campaign.isPending) return null;
  if (campaign.isError) {
    return <p className="form-error">Campaign evidence unavailable: {campaign.error.message}</p>;
  }
  const summary = campaign.data;
  return (
    <section aria-labelledby="campaign-heading">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Evidence-backed community alpha</p>
          <h2 id="campaign-heading">Proof-of-play campaign</h2>
        </div>
        <span className="count">
          {summary.verified}/{summary.attempts} verified · {Math.round(summary.success_rate * 100)}%
        </span>
      </div>
      <div className="game-grid">
        <article className="game-card" aria-label="Alpha gate">
          <span className="game-mark">{summary.gate_passed ? "✓" : "α"}</span>
          <span className="game-copy">
            <strong>
              {summary.gate_passed ? "8-of-10 gate passed" : "Collecting paired evidence"}
            </strong>
            <small>
              {summary.failed} failed · {summary.reports} privacy-safe reports
            </small>
          </span>
        </article>
        {summary.compatibility.map((row) => (
          <article
            className="game-card"
            key={`${row.game_id}:${row.platform}:${row.transport}:${row.nat}:${row.candidate}`}
          >
            <span className="game-mark">
              {row.verified}/{row.attempts}
            </span>
            <span className="game-copy">
              <strong>
                {row.game_id} · {row.transport}
              </strong>
              <small>
                {row.platform} · {row.nat} · {row.candidate}
              </small>
            </span>
          </article>
        ))}
      </div>
    </section>
  );
}

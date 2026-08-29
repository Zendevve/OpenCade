const status = document.querySelector("#status");
const cohorts = document.querySelector("#cohorts");
const refresh = document.querySelector("#refresh");

function apiOrigin() {
  const requested = new URLSearchParams(window.location.search).get("api");
  const candidate = requested || window.location.origin;
  const parsed = new URL(candidate);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
  if (parsed.protocol !== "https:" && !(parsed.protocol === "http:" && local)) {
    throw new Error("The compatibility API must use HTTPS unless it runs on this machine.");
  }
  return parsed.origin;
}

function cell(row, value) {
  const element = document.createElement("td");
  element.textContent = value;
  row.append(element);
}

function render(items) {
  cohorts.replaceChildren();
  const ranked = [...items].sort((left, right) => {
    const leftRate = left.attempts > 0 ? left.verified / left.attempts : 0;
    const rightRate = right.attempts > 0 ? right.verified / right.attempts : 0;
    return rightRate - leftRate || right.attempts - left.attempts;
  });
  for (const item of ranked) {
    const row = document.createElement("tr");
    cell(row, item.game_id);
    cell(row, item.platform);
    cell(row, `${item.adapter}${item.emulator_version ? ` ${item.emulator_version}` : ""}`);
    cell(row, `${item.native_route.replaceAll("_", " ")} · ${item.transport.replaceAll("_", " ")}`);
    const rate = item.attempts > 0 ? Math.round((item.verified / item.attempts) * 100) : 0;
    cell(row, `${rate}% · ${item.verified}/${item.attempts} rooms`);
    cohorts.append(row);
  }
}

async function load() {
  refresh.setAttribute("aria-busy", "true");
  status.textContent = "Loading compatibility evidence…";
  try {
    const response = await fetch(`${apiOrigin()}/api/v1/public/compatibility`, {
      headers: { Accept: "application/json" },
    });
    if (!response.ok) throw new Error(`Evidence service returned HTTP ${response.status}.`);
    const envelope = await response.json();
    const items = Array.isArray(envelope?.payload?.cohorts) ? envelope.payload.cohorts : [];
    render(items);
    const threshold = Number(envelope?.payload?.minimum_cohort_size) || 3;
    status.textContent = items.length
      ? `${items.length} privacy-safe cohorts · minimum ${threshold} rooms · generated ${new Date(envelope.payload.generated_at).toLocaleString()}`
      : `No cohort has reached the ${threshold}-room publication threshold yet.`;
  } catch (error) {
    status.textContent =
      error instanceof Error
        ? `${error.message} Add ?api=https://your-server.example to select the public API.`
        : "Compatibility evidence is unavailable. Try again.";
  } finally {
    refresh.removeAttribute("aria-busy");
  }
}

refresh.addEventListener("click", () => void load());
void load();

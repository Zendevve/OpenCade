import { useState } from "react";
import { runNetworkTest } from "../lib/native";

export default function DiagnosticsButton() {
  const [label, setLabel] = useState("Network test");
  const run = async () => {
    setLabel("Testing…");
    try {
      const result = await runNetworkTest();
      setLabel(result.rtt_ms === null ? "Server unreachable" : `${result.rtt_ms} ms`);
    } catch {
      setLabel("Test failed");
    }
  };
  return (
    <button className="text-button" onClick={() => void run()}>
      {label}
    </button>
  );
}

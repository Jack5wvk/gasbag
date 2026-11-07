// The heart trace. Nothing here is decorative: spikes sit at the real block of each breath, taken
// from the contract's log, and the line goes flat when the contract stops breathing.
const WIDTH = 1200;
const HEIGHT = 180;
const BASE = 118;
export const WINDOW_BLOCKS = 3600; // 36 minutes at 0.6 s a block

const NS = "http://www.w3.org/2000/svg";

function spike(x, amp) {
  return [
    [x - 16, BASE], [x - 12, BASE - 6], [x - 8, BASE], [x - 4, BASE + 7], [x, BASE - amp],
    [x + 5, BASE + 20], [x + 11, BASE], [x + 20, BASE - 9], [x + 28, BASE],
  ];
}

export function drawEcg(svg, { block, beats, status }) {
  const start = block - WINDOW_BLOCKS;
  const toX = (b) => ((b - start) / WINDOW_BLOCKS) * WIDTH;
  const shown = beats.filter((b) => b.block >= start && b.block <= block).sort((a, b) => a.block - b.block);

  const points = [[0, BASE]];
  let lastX = 0;
  for (const beat of shown) {
    const x = toX(beat.block);
    if (x - 18 <= lastX) continue;
    points.push(...spike(x, beat.kind === "Ignite" ? 44 : 84));
    lastX = x + 28;
  }
  points.push([WIDTH, BASE]);

  svg.replaceChildren();
  const line = document.createElementNS(NS, "path");
  line.setAttribute("class", "trace");
  line.setAttribute("vector-effect", "non-scaling-stroke");
  line.setAttribute("d", `M${points.map(([x, y]) => `${x.toFixed(1)} ${y}`).join(" L")}`);
  svg.appendChild(line);

  for (const beat of shown) {
    const x = toX(beat.block);
    const hit = document.createElementNS(NS, "rect");
    hit.setAttribute("x", x - 16);
    hit.setAttribute("y", 0);
    hit.setAttribute("width", 44);
    hit.setAttribute("height", HEIGHT);
    hit.setAttribute("class", "hit");
    const title = document.createElementNS(NS, "title");
    title.textContent = `${beat.kind === "Ignite" ? "Lung started" : "Breath"} by ${beat.who ?? "unknown"} at block ${beat.block}`;
    hit.appendChild(title);
    svg.appendChild(hit);
  }

  const dot = document.createElementNS(NS, "circle");
  dot.setAttribute("cx", WIDTH - 6);
  dot.setAttribute("cy", BASE);
  dot.setAttribute("r", 6);
  dot.setAttribute("class", status === "breathing" ? "cursor" : "cursor still");
  svg.appendChild(dot);
}

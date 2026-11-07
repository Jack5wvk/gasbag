// Gasbag: an original balloon character drawn in SVG. Its size follows the real fuel left in its
// lungs, and its face follows the contract's status.

const SKIN = "M150 26C224 26 270 82 263 150C256 216 192 250 150 262C108 250 44 216 37 150C30 82 76 26 150 26Z";

export const petMarkup = `
<svg viewBox="0 0 300 330" xmlns="http://www.w3.org/2000/svg">
  <g class="floor"><ellipse cx="150" cy="316" rx="92" ry="9"/></g>
  <path class="string" d="M150 274c-8 12 8 20 0 32"/>
  <g class="balloon">
    <path class="knot" d="M136 275l14-16 14 16c-9-4-19-4-28 0z"/>
    <path class="skin" d="${SKIN}"/>
    <ellipse class="shine" cx="96" cy="84" rx="26" ry="40" transform="rotate(26 96 84)"/>
    <g class="eyes">
      <g class="eye"><circle class="white" cx="112" cy="128" r="23"/><circle class="pupil" cx="117" cy="133" r="9"/></g>
      <g class="eye"><circle class="white" cx="188" cy="128" r="23"/><circle class="pupil" cx="183" cy="133" r="9"/></g>
    </g>
    <g class="lids"><path d="M86 112q26-20 52 2"/><path d="M162 114q26-22 52-2"/></g>
    <path class="mouth happy" d="M122 180q28 26 56 0"/>
    <ellipse class="mouth gasp" cx="150" cy="190" rx="17" ry="23"/>
    <path class="mouth worried" d="M124 194q26-18 52 0"/>
    <path class="sweat" d="M238 96c9 12 11 18 5 23c-7 5-15-1-13-9c1-5 4-9 8-14z"/>
    <g class="cheeks"><ellipse cx="82" cy="170" rx="14" ry="8"/><ellipse cx="218" cy="170" rx="14" ry="8"/></g>
  </g>
  <g class="corpse">
    <path class="skin" d="M42 300c12-44 54-72 108-72s96 28 108 72c-34 22-182 22-216 0z"/>
    <path class="wrinkle" d="M84 288c10-14 18-20 30-24M214 290c-8-13-18-20-29-25M150 300v-20"/>
    <path class="dead-eye" d="M100 256l18 18m0-18l-18 18M182 256l18 18m0-18l-18 18"/>
  </g>
</svg>`;

/** Accepts status from the contract and fullness from 0 to 1. */
export function setPet(root, status, fullness) {
  const svg = root.querySelector("svg");
  if (!svg) return;
  let mood = status;
  if (status === "breathing" && fullness < 0.25) mood = "worried";
  svg.dataset.mood = mood;
  const scale = 0.62 + 0.38 * Math.max(0, Math.min(1, fullness));
  root.style.setProperty("--fill", scale.toFixed(3));
}

/** A puff for each real breath. */
export function puff(root) {
  const svg = root.querySelector("svg");
  if (!svg) return;
  svg.classList.remove("puff");
  void svg.getBoundingClientRect();
  svg.classList.add("puff");
}

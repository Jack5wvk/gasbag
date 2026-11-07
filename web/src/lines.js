// Things Gasbag says when its voice isn't connected. Anything spoken on chain replaces these.
const LINES = {
  breathing: [
    "In. Out. In. Out. Nobody is pressing a button. This is all me.",
    "I am a process, not a price. Please don't check the price.",
    "Every two minutes I wake up, panic, and go back to sleep.",
    "{lung} is keeping me alive and I will tell everyone about it.",
    "They said memecoins have no fundamentals. I have lungs.",
    "A contract that wakes itself up. Your doctor hates this one trick.",
  ],
  worried: [
    "Is it me, or is the gas running a bit thin in here?",
    "I don't want to alarm anyone, but I am deflating.",
    "Someone hold my string. Someone. Anyone.",
  ],
  apnea: [
    "Hhhhhh. Hhhh. Tell {lung} I said hhhh.",
    "No lungs. The bounty is growing. Are you all just waiting? That is cold.",
    "Who blinks first? Not me, I've got no blood pressure.",
    "I can see the estate from here.",
  ],
  dead: [
    "It died as it lived: out of gas.",
    "Every memecoin dies. This one left a will.",
    "The bag holders have been notified.",
  ],
};

export function line(status, lung) {
  const pool = LINES[status] ?? LINES.breathing;
  const pick = pool[Math.floor(Date.now() / 30000) % pool.length];
  return pick.replace("{lung}", lung || "somebody");
}

// A round: what one is, who it was judged against, and which ones count as a win.
//
//   import { OPPONENTS, nextRound, round, rounds, wonBefore } from './rounds.mjs'
//
// `progress/rounds/<piece>-r<N>.json` is the Gate's ledger, and two things read it: `tools/gate
// judge`, which writes one and has to know whether this Piece had been won before, and
// `tools/progress.mjs`, which draws them all. What a round means lives here so that neither owns
// it — the page would otherwise be importing the whole judging command for a caption, and the two
// could disagree about what a win is.
//
// The shape is the gauntlet's, because the rounds it wrote are still in the ledger and still drawn.
// A native round adds three keys and changes none: `opponent`, `build` and `states`.
import fs from 'node:fs';
import path from 'node:path';

// What each opponent is called in prose. The key is what a round records; the value is what the
// progress page prints and what the line saying a won Piece has been lost calls it.
//
// A round with no `opponent` at all is one of the gauntlet's, judged against iA Writer — see
// [`opponentName`], which is where that default lives.
// `mac-native` is the Design oracle — iA Writer for Mac, captured in `ref/ia/shots/mac-native/` and
// paired as a crop (ADR 0015) — and `mixed` is a Piece part-way through: some of its states are
// judged against a `docs/design.md` row and the rest still against `legacy/`.
export const OPPONENTS = { oracle: 'Parity oracle', 'mac-native': 'Design oracle', mixed: 'Parity and Design oracles' };

// Who a round was judged against, as words. A round from before this command carries no
// `opponent`, and every one of those is the JavaScript app's gauntlet against iA Writer.
export function opponentName(r) {
  if (!r || !r.opponent) return 'iA Writer';
  return OPPONENTS[r.opponent] || r.opponent;
}

// Every round recorded for a Piece, oldest first.
export function rounds(root, piece) {
  const dir = path.join(root, 'progress/rounds');
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir)
    .filter((f) => f.startsWith(`${piece}-r`) && f.endsWith('.json'))
    .map((f) => JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')))
    .filter((r) => r.piece === piece)
    .sort((a, b) => a.round - b.round);
}

// The round a run about to be judged is: one past the highest already recorded, whoever recorded it.
export function nextRound(recorded) {
  return recorded.reduce((n, r) => Math.max(n, Number(r.round) || 0), 0) + 1;
}

// The round that won this Piece against a native opponent, or null.
//
// "A Piece once won is never lost" reads only rounds that carry `opponent`. The rounds the gauntlet
// left are the JavaScript app's against iA Writer — the reference era, the thing the native app is
// being built to match — and a native Piece that has never been judged has not been won, however
// many r1 verdicts sit beside it saying "ours".
export function wonBefore(recorded) {
  return recorded.filter((r) => r.opponent && r.winner === 'ours').pop() || null;
}

// The state whose verdict the round's single-pair keys are filled from: the first one lost, and the
// first one of all if none was.
//
// The gauntlet judged one pair per Piece, and the round file still has room for exactly one.
// Keeping that shape is what lets `tools/progress.mjs` go on reading every round ever written;
// filling it from the decisive state is what stops a Piece that lost on its third state showing the
// first state's comfortable verdict on the page.
export function decisive(judged) {
  return judged.find((s) => s.winner === 'theirs') || judged[0];
}

// The round file, in the shape every round since the gauntlet has been written in, plus the three
// keys a native round needs that a gauntlet round did not: who the opponent was, which build of
// ours it was, and what happened at each judged state.
//
// `headline` fills the single-pair keys itself, for a run whose answer is not any one of its
// judged states'. A blind round's is: one state lost, and that state's verdict is the round's.
// `tools/gate judge latency` is twelve regimes and one bench summary, and the pair the round names
// is that summary against the oracle's report — which is not a thing any single regime says.
export function round({ piece, number, judged, opponent, build, oracle, note, at, headline }) {
  const head = headline || decisive(judged);
  return {
    piece,
    round: number,
    winner: judged.every((s) => s.winner === 'ours') ? 'ours' : 'theirs',
    margin: head.margin,
    gap: head.gap,
    gapTheirs: head.gapTheirs,
    verdict: head.verdict,
    oursShot: head.ours,
    theirsShot: head.theirs,
    refSource: oracle,
    builderNote: note || '',
    secondary: head.secondary,
    opponent,
    build,
    states: judged,
    at,
  };
}

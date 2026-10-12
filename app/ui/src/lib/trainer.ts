/** How the speed trainer raises the speed as a loop goes round. */
export interface Trainer {
  /** Percentage points added to the speed each time it is raised. */
  step: number;
  /** Times round the loop at each speed. */
  every: number;
  /** The speed it stops at, as a percentage of the tempo written. */
  target: number;
}

/**
 * The speed for the time round after time `pass` (counted from 1), which is played at
 * `speed`: a step faster once it has been round as often as it should at this speed, and
 * never past the target.
 */
export function nextSpeed(trainer: Trainer, speed: number, pass: number): number {
  if (speed >= trainer.target || pass % trainer.every !== 0) return speed;
  return Math.min(trainer.target, speed + trainer.step);
}

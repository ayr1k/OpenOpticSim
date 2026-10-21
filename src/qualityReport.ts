export function mtfAt(
  values: number[],
  step: number,
  frequency: number,
): number | null {
  if (!(step > 0) || !Number.isFinite(frequency) || frequency < 0) return null;
  const at = frequency / step,
    lo = Math.floor(at),
    hi = Math.ceil(at);
  if (hi >= values.length) return null;
  return values[lo] + (values[hi] - values[lo]) * (at - lo);
}

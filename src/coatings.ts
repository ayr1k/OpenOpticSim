export interface CoatingLayer {
  name: string;
  index: number;
  thickness_nm: number;
}
export function validCoating(v: unknown): v is CoatingLayer[] {
  return (
    Array.isArray(v) &&
    v.length <= 32 &&
    v.every(
      (l) =>
        l &&
        typeof l.name === "string" &&
        Number.isFinite(l.index) &&
        l.index >= 1 &&
        l.index <= 10 &&
        Number.isFinite(l.thickness_nm) &&
        l.thickness_nm >= 0 &&
        l.thickness_nm <= 10000,
    )
  );
}

/**
 * Amari - High-performance Geometric Algebra library for TypeScript
 *
 * This library provides TypeScript bindings for the Amari Rust crate.
 *
 * Supports:
 * - Arbitrary Clifford algebra signatures Cl(p,q,r) via `Multivector`
 * - 8 fast-path aliases for common signatures:
 *     GA  = euclidean3D()   // Cl(3,0,0)
 *     ST  = spacetime2p1()  // Cl(2,1,0)
 *     MINK= minkowski()     // Cl(3,1,0)
 *     PL  = planar()        // Cl(2,0,0)
 *     QUAT= quaternion()    // Cl(0,3,0)
 *     CGA = conformal()     // Cl(4,1,0)
 *     P5D = euclidean5D()   // Cl(5,0,0)
 *     S2D = split2D()       // Cl(1,1,0)
 */

import init, {
  WasmGenericMultivector,
  WasmGenericRotor,
  WasmMultivector300,
  WasmRotor300,
  WasmMultivector210,
  WasmRotor210,
  WasmMultivector310,
  WasmRotor310,
  WasmMultivector200,
  WasmRotor200,
  WasmMultivector030,
  WasmRotor030,
  WasmMultivector410,
  WasmRotor410,
  WasmMultivector500,
  WasmRotor500,
  WasmMultivector110,
  WasmRotor110,
  BatchOperations,
} from '../pkg/amari_wasm.js';

/**
 * Initialize the WASM module
 */
export async function initAmari(): Promise<void> {
  await init();
}

// ========================================================================
// Handle type — returned by fast-path factory methods.
// Bundles (p, q, r) with named basis-vector constructors.
// ========================================================================

/** Signature-aware algebra handle with named basis constructors. */
export class AlgebraHandle {
  constructor(
    public readonly p: number,
    public readonly q: number,
    public readonly r: number,
  ) {}

  get dim(): number { return this.p + this.q + this.r; }
  get basisCount(): number { return 1 << this.dim; }

  scalar(value: number): Multivector {
    return new Multivector(WasmGenericMultivector.scalar(this.p, this.q, this.r, value));
  }

  zero(): Multivector {
    return new Multivector(new WasmGenericMultivector(this.p, this.q, this.r));
  }

  basisVector(index: number): Multivector {
    return new Multivector(
      WasmGenericMultivector.basisVector(this.p, this.q, this.r, index),
    );
  }

  builder(): MultivectorBuilder {
    return new MultivectorBuilder(this.p, this.q, this.r);
  }

  /** Create from coefficients. */
  fromCoefficients(coefficients: number[]): Multivector {
    return new Multivector(
      WasmGenericMultivector.fromCoefficients(
        this.p, this.q, this.r,
        new Float64Array(coefficients),
      ),
    );
  }

  /** Pre-built basis vectors (lazily initialized). */
  private _e0?: Multivector; private _e1?: Multivector;
  private _e2?: Multivector; private _e3?: Multivector; private _e4?: Multivector;

  e0() { return this._e0 ??= this.basisVector(0); }
  e1() { return this._e1 ??= this.basisVector(1); }
  e2() { return this._e2 ??= this.basisVector(2); }
  e3() { return this._e3 ??= this.basisVector(3); }
  e4() { return this._e4 ??= this.basisVector(4); }
}

// ========================================================================
// Basis blade indices (binary representation, standard for all signatures)
// ========================================================================

/** Basis blade indices using binary representation (standard). */
export enum BasisBlade {
  Scalar = 0,
  E1 = 1,   E2 = 2,   E3 = 4,   E4 = 8,   E5 = 16,
}

// ========================================================================
// Multivector — generic over signature (p, q, r)
// ========================================================================

export class Multivector {
  private inner: WasmGenericMultivector;

  /**
   * Create a multivector in Cl(p, q, r).
   *
   * @param p - basis vectors squaring to +1
   * @param q - basis vectors squaring to -1
   * @param r - basis vectors squaring to 0
   *
   * Defaults to Cl(3, 0, 0) for backward compatibility.
   */
  constructor(p?: WasmGenericMultivector | number, q?: number, r?: number) {
    if (p instanceof WasmGenericMultivector) {
      this.inner = p;
    } else {
      this.inner = new WasmGenericMultivector(
        (p as number) ?? 3,
        q ?? 0,
        r ?? 0,
      );
    }
  }

  /** @internal */
  get raw(): WasmGenericMultivector { return this.inner; }

  // ---- accessors ----

  get p(): number { return this.inner.p; }
  get q(): number { return this.inner.q; }
  get r(): number { return this.inner.r; }
  get dim(): number { return this.inner.dim; }
  get basisCount(): number { return this.inner.basisCount; }

  // ---- fast-path factory methods ----

  /** Cl(3,0,0) — 3D Euclidean geometric algebra */
  static euclidean3D(): AlgebraHandle { return new AlgebraHandle(3, 0, 0); }

  /** Cl(2,1,0) — 2+1 spacetime algebra */
  static spacetime2p1(): AlgebraHandle { return new AlgebraHandle(2, 1, 0); }

  /** Cl(3,1,0) — 3+1 Minkowski spacetime */
  static minkowski(): AlgebraHandle { return new AlgebraHandle(3, 1, 0); }

  /** Cl(2,0,0) — 2D planar Euclidean (complex-number algebra) */
  static planar(): AlgebraHandle { return new AlgebraHandle(2, 0, 0); }

  /** Cl(0,3,0) — pure quaternion algebra (all negative-definite) */
  static quaternion(): AlgebraHandle { return new AlgebraHandle(0, 3, 0); }

  /** Cl(4,1,0) — conformal geometric algebra */
  static conformal(): AlgebraHandle { return new AlgebraHandle(4, 1, 0); }

  /** Cl(5,0,0) — 5D Euclidean */
  static euclidean5D(): AlgebraHandle { return new AlgebraHandle(5, 0, 0); }

  /** Cl(1,1,0) — split-complex / 1+1 spacetime */
  static split2D(): AlgebraHandle { return new AlgebraHandle(1, 1, 0); }

  // ---- coefficient access ----

  getCoefficients(): number[] {
    return Array.from(this.inner.getCoefficients());
  }

  getCoefficient(index: number): number {
    return this.inner.getCoefficient(index);
  }

  setCoefficient(index: number, value: number): this {
    this.inner.setCoefficient(index, value);
    return this;
  }

  // ---- operations ----

  geometricProduct(other: Multivector): Multivector {
    return new Multivector(this.inner.geometricProduct(other.inner));
  }

  innerProduct(other: Multivector): Multivector {
    return new Multivector(this.inner.innerProduct(other.inner));
  }

  outerProduct(other: Multivector): Multivector {
    return new Multivector(this.inner.outerProduct(other.inner));
  }

  scalarProduct(other: Multivector): number {
    return this.inner.scalarProduct(other.inner);
  }

  reverse(): Multivector {
    return new Multivector(this.inner.reverse());
  }

  gradeProjection(grade: number): Multivector {
    return new Multivector(this.inner.gradeProjection(grade));
  }

  exp(): Multivector {
    return new Multivector(this.inner.exp());
  }

  magnitude(): number {
    return this.inner.magnitude();
  }

  norm(): number {
    return this.magnitude();
  }

  normalize(): Multivector {
    return new Multivector(this.inner.normalize());
  }

  inverse(): Multivector {
    return new Multivector(this.inner.inverse());
  }

  add(other: Multivector): Multivector {
    return new Multivector(this.inner.add(other.inner));
  }

  sub(other: Multivector): Multivector {
    return new Multivector(this.inner.sub(other.inner));
  }

  scale(scalar: number): Multivector {
    return new Multivector(this.inner.scale(scalar));
  }

  toString(): string {
    const coeffs = this.getCoefficients();
    const terms: string[] = [];
    for (let i = 0; i < coeffs.length; i++) {
      if (Math.abs(coeffs[i]) > 1e-10) {
        terms.push(`${coeffs[i].toFixed(3)}e${i}`);
      }
    }
    return terms.length > 0 ? terms.join(' + ') : '0';
  }
}

// ========================================================================
// Rotor — generic over signature
// ========================================================================

export class Rotor {
  private inner: WasmGenericRotor;

  constructor(inner: WasmGenericRotor) {
    this.inner = inner;
  }

  static fromBivector(bivector: Multivector, angle: number): Rotor {
    return new Rotor(WasmGenericRotor.fromBivector(bivector.raw, angle));
  }

  apply(mv: Multivector): Multivector {
    return new Multivector(this.inner.apply(mv.raw));
  }

  compose(other: Rotor): Rotor {
    return new Rotor(this.inner.compose(other.inner));
  }

  inverse(): Rotor {
    return new Rotor(this.inner.inverse());
  }
}

// ========================================================================
// MultivectorBuilder
// ========================================================================

export class MultivectorBuilder {
  private coefficients: number[];

  constructor(p: number, q: number, r: number) {
    const bc = 1 << (p + q + r);
    this.coefficients = new Array(bc).fill(0);
  }

  set(index: number, value: number): this {
    this.coefficients[index] = value;
    return this;
  }

  build(mv: Multivector): Multivector {
    mv.setCoefficient(0, 0); // trigger re-init? no, we use fromCoefficients
    // Use fromCoefficients on the same algebra
    const result = new Multivector(mv.p, mv.q, mv.r);
    for (let i = 0; i < this.coefficients.length; i++) {
      result.setCoefficient(i, this.coefficients[i]);
    }
    return result;
  }
}

// ========================================================================
// Batch operations
// ========================================================================

export class BatchOps {
  /** Batch geometric product for a given signature. */
  static async geometricProduct(
    p: number, q: number, r: number,
    a: Float64Array, b: Float64Array,
  ): Promise<Float64Array> {
    const result = await BatchOperations.batchGeometricProduct(p, q, r, a, b);
    return new Float64Array(result);
  }

  /** Batch addition (signature-independent). */
  static async add(a: Float64Array, b: Float64Array): Promise<Float64Array> {
    const result = await BatchOperations.batchAdd(a, b);
    return new Float64Array(result);
  }
}

// ========================================================================
// Backward-compatible convenience exports
// ========================================================================

/** Cl(3,0,0) convenience constructors (backward compatible). */
export const GA = Multivector.euclidean3D();

/** Cl(2,1,0) convenience constructors. */
export const ST = Multivector.spacetime2p1();

/** Cl(3,1,0) Minkowski spacetime. */
export const MINK = Multivector.minkowski();

/** Cl(2,0,0) 2D planar. */
export const PL = Multivector.planar();

/** Cl(0,3,0) Quaternion algebra. */
export const QUAT = Multivector.quaternion();

/** Cl(4,1,0) Conformal GA. */
export const CGA = Multivector.conformal();

/** Cl(5,0,0) 5D Euclidean. */
export const P5D = Multivector.euclidean5D();

/** Cl(1,1,0) Split-complex / 1+1 spacetime. */
export const S2D = Multivector.split2D();

// Re-export classes
export {
  initAmari as init,
};

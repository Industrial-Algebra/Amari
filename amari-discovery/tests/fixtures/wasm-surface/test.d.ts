/* tslint:disable */
/* eslint-disable */

/**
 * A fast-path multivector for Cl(3,0,0).
 *
 * Used for 3D Euclidean geometric algebra computations.
 */
export class WasmMultivector300 {
    free(): void;
    [Symbol.dispose](): void;
    add(other: WasmMultivector300): WasmMultivector300;
    /**
     * Compute the geometric product.
     *
     * a * b where * is the Clifford geometric product.
     */
    geometricProduct(other: WasmMultivector300): WasmMultivector300;
    /**
     * Create a basis vector (0-indexed).
     */
    static basisVector(index: number): WasmMultivector300;
    exp(): WasmMultivector300;
    /**
     * Create from a Float64Array of coefficients.
     */
    static fromCoefficients(coefficients: Float64Array): WasmMultivector300;
    getCoefficient(index: number): number;
    getCoefficients(): Float64Array;
    gradeProjection(grade: number): WasmMultivector300;
    innerProduct(other: WasmMultivector300): WasmMultivector300;
    inverse(): WasmMultivector300;
    magnitude(): number;
    /**
     * Create a new zero multivector.
     */
    constructor();
    norm(): number;
    normalize(): WasmMultivector300;
    outerProduct(other: WasmMultivector300): WasmMultivector300;
    reverse(): WasmMultivector300;
    /**
     * Create a scalar multivector.
     */
    static scalar(value: number): WasmMultivector300;
    scalarProduct(other: WasmMultivector300): number;
    scale(scalar: number): WasmMultivector300;
    setCoefficient(index: number, value: number): void;
    sub(other: WasmMultivector300): WasmMultivector300;
    readonly dim: number;
}

/**
 * Generic multivector with arbitrary signature.
 */
export class WasmGenericMultivector {
    free(): void;
    [Symbol.dispose](): void;
    add(other: WasmGenericMultivector): WasmGenericMultivector;
    static basisVector(p: number, q: number, r: number, index: number): WasmGenericMultivector;
    exp(): WasmGenericMultivector;
    static fromCoefficients(p: number, q: number, r: number, coefficients: Float64Array): WasmGenericMultivector;
    geometricProduct(other: WasmGenericMultivector): WasmGenericMultivector;
    getCoefficient(index: number): number;
    getCoefficients(): Float64Array;
    /**
     * Project onto a specific grade.
     */
    gradeProjection(grade: number): WasmGenericMultivector;
    innerProduct(other: WasmGenericMultivector): WasmGenericMultivector;
    inverse(): WasmGenericMultivector;
    magnitude(): number;
    /**
     * Create a new zero multivector in Cl(p, q, r).
     */
    constructor(p: number, q: number, r: number);
    norm(): number;
    normalize(): WasmGenericMultivector;
    outerProduct(other: WasmGenericMultivector): WasmGenericMultivector;
    reverse(): WasmGenericMultivector;
    static scalar(p: number, q: number, r: number, value: number): WasmGenericMultivector;
    scalarProduct(other: WasmGenericMultivector): number;
    scale(scalar: number): WasmGenericMultivector;
    setCoefficient(index: number, value: number): void;
    sub(other: WasmGenericMultivector): WasmGenericMultivector;
    readonly basisCount: number;
    readonly dim: number;
    readonly p: number;
    readonly q: number;
    readonly r: number;
}

/**
 * Generic rotor for arbitrary-signature geometric algebra.
 */
export class WasmGenericRotor {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Apply rotor to a multivector: R * v * R†
     */
    apply(mv: WasmGenericMultivector): WasmGenericMultivector;
    /**
     * Compose two rotors.
     */
    compose(other: WasmGenericRotor): WasmGenericRotor;
    /**
     * Create a rotor from a bivector and angle.
     *
     * Only supported for DIM ≤ 6 (match table).
     */
    static fromBivector(bivector: WasmGenericMultivector, angle: number): WasmGenericRotor;
    /**
     * Get inverse rotor.
     */
    inverse(): WasmGenericRotor;
}

/**
 * Fast-path rotor for Cl(3,0,0).
 */
export class WasmRotor300 {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Apply rotor to a multivector.
     */
    apply(mv: WasmMultivector300): WasmMultivector300;
    /**
     * Compose two rotors.
     */
    compose(other: WasmRotor300): WasmRotor300;
    /**
     * Create a rotor from a bivector and angle.
     */
    static fromBivector(bivector: WasmMultivector300, angle: number): WasmRotor300;
    /**
     * Get inverse rotor.
     */
    inverse(): WasmRotor300;
}

/**
 * Counting measure for WASM.
 */
export class WasmCountingMeasure {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Count elements in an array.
     */
    count(values: Float64Array): number;
}

/**
 * A pointless class with no meaningful content — testing parser resilience.
 */
export class EmptyShell {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
}

/**
 * A class with nested generic types.
 */
export class NestedContainer {
    mapTransform(data: Map<string, Array<Float64Array>>): Map<string, Array<Float64Array>>;
    tupleResult(input: [number, string, boolean]): [string, number];
}

/**
 * Integration method enum.
 */
export enum WasmIntegrationMethod {
    /**
     * Riemann sum.
     */
    Riemann = 0,
    /**
     * Monte Carlo.
     */
    MonteCarlo = 1,
    /**
     * Trapezoidal rule.
     */
    Trapezoidal = 2,
}

/**
 * Initialize bindings.
 */
export function init(): void;

/**
 * Compute expectation via Monte Carlo.
 */
export function expectation(f: Function, a: number, b: number, samples: number): number;

/**
 * Convert velocity to Lorentz factor.
 */
export function velocity_to_gamma(velocity_magnitude: number): number;

/**
 * Init input type alias.
 */
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Sync init input.
 */
export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Init output interface.
 */
export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly wasm_multivector300_geometric_product: (a: number, b: number, c: number) => number;
}

/**
 * Re-export aliases.
 */
export { WasmMultivector300, WasmGenericRotor as GenericRotor };

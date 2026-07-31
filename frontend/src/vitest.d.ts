declare module "vitest" {
  interface Matchers {
    not: Matchers;
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toContain(expected: unknown): void;
    toHaveLength(expected: number): void;
    toBeTruthy(): void;
    toBeFalsy(): void;
    toBeNull(): void;
    toBeUndefined(): void;
    toBeDefined(): void;
    toMatchObject(expected: unknown): void;
    toHaveBeenCalled(): void;
    toHaveBeenCalledTimes(expected: number): void;
  }

  interface Mock<TArgs extends unknown[] = unknown[], TResult = unknown> {
    (...args: TArgs): TResult;
    mock: { calls: TArgs[] };
    mockReset(): Mock<TArgs, TResult>;
    mockReturnValue(value: TResult): Mock<TArgs, TResult>;
    mockResolvedValue(value: Awaited<TResult>): Mock<TArgs, TResult>;
    mockResolvedValueOnce(value: Awaited<TResult>): Mock<TArgs, TResult>;
    mockImplementation(fn: (...args: TArgs) => TResult): Mock<TArgs, TResult>;
    mockImplementationOnce(fn: (...args: TArgs) => TResult): Mock<TArgs, TResult>;
  }

  interface VitestUtils {
    fn<TArgs extends unknown[] = unknown[], TResult = unknown>(
      impl?: (...args: TArgs) => TResult,
    ): Mock<TArgs, TResult>;
    hoisted<T>(factory: () => T): T;
    mock(path: string, factory: () => unknown): void;
    clearAllMocks(): void;
    resetModules(): void;
    useFakeTimers(): void;
    useRealTimers(): void;
    clearAllTimers(): void;
    stubGlobal(name: string, value: unknown): void;
    unstubAllGlobals(): void;
  }

  export function describe(name: string, fn: () => void): void;
  export function it(name: string, fn: () => void | Promise<void>): void;
  export function expect<T = unknown>(actual: T): Matchers;
  export function beforeEach(fn: () => void | Promise<void>): void;
  export function afterEach(fn: () => void | Promise<void>): void;
  export const vi: VitestUtils;
}

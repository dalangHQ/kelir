// see: cans/03-sdk-architecture.md#TypeScript-SDK

export interface Schema<T = unknown> {
  readonly __type?: T;
}

export type Infer<S> = S extends Schema<infer T> ? T : unknown;

// TODO (L3): emit real TypeBox schemas from the t.* builders so builder.boot() fails closed without the typebox dependency.
export const t = {
  String(): Schema<string> {
    return {};
  },
  Number(): Schema<number> {
    return {};
  },
  Boolean(): Schema<boolean> {
    return {};
  },
  Literal<V extends string | number | boolean>(value: V): Schema<V> {
    void value;
    return {};
  },
  Optional<S extends Schema<any>>(schema: S): Schema<Infer<S> | undefined> {
    void schema;
    return {};
  },
  Array<S extends Schema<any>>(schema: S): Schema<readonly Infer<S>[]> {
    void schema;
    return {};
  },
  Union<const T extends readonly Schema<any>[]>(
    schemas: T,
  ): Schema<Infer<T[number]>> {
    void schemas;
    return {};
  },
  Object<P extends Record<string, Schema<any>>>(
    props: P,
  ): Schema<{ [K in keyof P]: Infer<P[K]> }> {
    void props;
    return {};
  },
};

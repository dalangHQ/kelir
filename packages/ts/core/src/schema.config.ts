// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { Type, type TAnySchema } from "@sinclair/typebox";
import { Value } from "@sinclair/typebox/value";
import { EConfig } from "./errors.sdk.js";

export interface Schema<T = unknown> {
  readonly __type?: T;
  readonly typebox: TAnySchema;
}

export type Infer<S> = S extends Schema<infer T> ? T : unknown;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function validateSchema<S extends Schema<any>>(
  schema: S,
  value: unknown,
): Infer<S> {
  if (!Value.Check(schema.typebox, value)) {
    let detail = "schema violation";
    for (const error of Value.Errors(schema.typebox, value)) {
      detail = `schema violation at ${error.path === "" ? "/" : error.path}: ${error.message}`;
      break;
    }
    throw new EConfig(detail);
  }
  return value as Infer<S>;
}

export const t = {
  String(): Schema<string> {
    return { typebox: Type.String() };
  },
  Number(): Schema<number> {
    return { typebox: Type.Number() };
  },
  Boolean(): Schema<boolean> {
    return { typebox: Type.Boolean() };
  },
  Literal<V extends string | number | boolean>(value: V): Schema<V> {
    return { typebox: Type.Literal(value) };
  },
  Optional<S extends Schema<any>>(schema: S): Schema<Infer<S> | undefined> {
    return { typebox: Type.Optional(schema.typebox) };
  },
  Array<S extends Schema<any>>(schema: S): Schema<readonly Infer<S>[]> {
    return { typebox: Type.Array(schema.typebox) };
  },
  Union<const T extends readonly Schema<any>[]>(
    schemas: T,
  ): Schema<Infer<T[number]>> {
    return {
      typebox: Type.Union(schemas.map((schema) => schema.typebox)),
    };
  },
  Object<P extends Record<string, Schema<any>>>(
    props: P,
  ): Schema<{ [K in keyof P]: Infer<P[K]> }> {
    const properties: Record<string, TAnySchema> = {};
    for (const [key, prop] of Object.entries(props)) {
      properties[key] = prop.typebox;
    }
    return { typebox: Type.Object(properties) };
  },
};

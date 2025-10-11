import { z } from 'zod';
import { ApiResponse } from './api/api';

export const DateStringSchema = z
    .string()
    .refine((str) => !isNaN(Date.parse(str)), {
        message: 'Invalid date format'
    })
    .transform((str) => new Date(str));

export const OptionalSchema = <T extends z.ZodTypeAny>(schema: T) =>
    schema
        .optional()
        .nullable()
        .transform((value) => (value === null ? undefined : value));

export async function parseResponse<T extends z.ZodObject>(schema: T, response: ApiResponse): Promise<z.infer<T>> {
    const data = await response.json();
    try {
        return schema.parse(data);
    } catch (err) {
        console.error('Failed to parse response', data, 'with error', err);
        const error = err as z.ZodError;
        throw new Error(error.message);
    }
}

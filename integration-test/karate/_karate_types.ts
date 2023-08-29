import { defineParameterType, World } from '@cucumber/cucumber';
import chai from './_chai';
import { KarateState } from './_karate_state';
import { KarateWorld } from './karate';

/// Helper to get KarateState from this (World) where @binding is not working for some reason
/// A common use is the transformer function of registered types
export const karate = function (world: any): KarateState {
    const karate = world?.__SCENARIO_CONTEXT?._activeObjects?.get(
        KarateState.prototype
    );
    chai.assert(world instanceof World, 'Provided context is not a World');
    chai.assert(karate !== undefined, 'Missing "use karate" step');
    chai.assert(
        karate instanceof KarateState,
        'Karate state is corrupted, not an instance of KarateState.'
    );
    return karate;
};

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

class HttpMethodType {
    name = 'HttpMethod';
    regexp = new RegExp('GET|POST|PUT|PATCH| DELETE');

    transformer(method: string): HttpMethod {
        return method as HttpMethod;
    }
}
defineParameterType(new HttpMethodType());

class IdentType {
    name = 'ident';
    regexp = /([_a-zA-Z][_a-zA-Z0-9]{0,30})/;

    transformer(ident: string): string {
        return ident;
    }
}
defineParameterType(new IdentType());

/// expression resulting a `string` evaluated in the transformer
class StringExprType {
    name = 'stringExpr';
    regexp = [/\'(.*)\'/, /\"(.*)\"/, /\((.*)\)/];

    async transformer(
        singleQ: string,
        doubleQ: string,
        expr: string
    ): Promise<string> {
        if (expr) {
            const value = await karate(this).evalAsyncExpr(expr);
            chai.assert(
                typeof value === 'string',
                `String expected, got: ${typeof value}`
            );
            return value;
        } else {
            return singleQ ?? doubleQ;
        }
    }
}
defineParameterType(new StringExprType());

/// expression resulting an `Record<string,string>` evaluated in the transformer
class ParamExprType {
    name = 'paramExpr';
    regexp = [/(\{.*\})/, /\((.*)\)/];

    async transformer(
        params: string,
        expr: string
    ): Promise<Record<string, string>> {
        if (params) {
            //todo: validated Record<string,string> parse
            return JSON.parse(params);
        } else {
            const value = await karate(this).evalAsyncExpr(expr);
            chai.assert(
                typeof value === 'object',
                `(Param) object expected, got: ${typeof value}`
            );
            return value;
        }
    }
}
defineParameterType(new ParamExprType());

/// expression resulting an (json) `object` evaluated in the transformer
class JsonExprType {
    name = 'jsonExpr';
    regexp = [/(\{.*\})/, /\((.*)\)/];

    async transformer(json: string, expr: string): Promise<any> {
        if (json) {
            return JSON.parse(json);
        } else {
            const value = await karate(this).evalAsyncExpr(expr);
            chai.assert(
                typeof value === 'object',
                `(Json) object expected, got: ${typeof value}`
            );
            return value;
        }
    }
}
defineParameterType(new JsonExprType());

/// Any string, similar to the {} pattern, but enclosed in parenthesis.
/// Expression is not evaluated, the string is returned
class ExprType {
    name = 'expr';
    regexp = /\((.*)\)/;

    transformer(expr: string): string {
        return expr;
    }
}
defineParameterType(new ExprType());

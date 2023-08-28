interface String {
    format(kv: Record<string, string>): string;
    formatExpr(context: any): string;
}

if (!String.prototype.format) {
    String.prototype.format = function (kv) {
        return this.replace(
            /\${([_a-zA-Z][_a-zA-Z0-9]*)}/,
            function (_match, key) {
                return kv[key] ?? `{UNDEFINED<${key}>}`;
            }
        );
    };

    String.prototype.formatExpr = function (context) {
        return this.replace(/\${(.*)}/, function (_match, expr) {
            const value = new Function('karate', `return ${expr}`)(context);
            return value ?? `{UNDEFINED<${expr}>}`;
        });
    };
}

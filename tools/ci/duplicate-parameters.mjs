// Preserve duplicate-parameter coverage missing from native Oxlint1.90.0.
// Unknown binding shapes fail the lint run instead of silently skipping them.
function bindings(pattern, visit) {
  if (pattern === null) return;
  switch (pattern.type) {
    case "Identifier":
      visit(pattern);
      return;
    case "AssignmentPattern":
      bindings(pattern.left, visit);
      return;
    case "RestElement":
      bindings(pattern.argument, visit);
      return;
    case "ArrayPattern":
      for (const entry of pattern.elements) bindings(entry, visit);
      return;
    case "ObjectPattern":
      for (const property of pattern.properties) {
        if (property.type === "RestElement") bindings(property.argument, visit);
        else if (property.type === "Property") bindings(property.value, visit);
        else throw new Error(`Unsupported parameter property: ${property.type}`);
      }
      return;
    case "TSParameterProperty":
      bindings(pattern.parameter, visit);
      return;
    default:
      throw new Error(`Unsupported parameter binding: ${pattern.type}`);
  }
}

export default {
  meta: { name: "backplane" },
  rules: {
    "no-dupe-args": {
      meta: {
        type: "problem",
        schema: [],
        messages: { duplicate: "Duplicate parameter '{{name}}'." },
      },
      create(context) {
        function check(node) {
          const seen = new Set();
          for (const parameter of node.params) {
            bindings(parameter, (identifier) => {
              if (seen.has(identifier.name)) {
                context.report({
                  node: identifier,
                  messageId: "duplicate",
                  data: { name: identifier.name },
                });
              }
              seen.add(identifier.name);
            });
          }
        }
        return {
          FunctionDeclaration: check,
          FunctionExpression: check,
          ArrowFunctionExpression: check,
        };
      },
    },
  },
};

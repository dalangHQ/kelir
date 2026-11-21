#!/usr/bin/env node
// see: cans/01-architecture.md#Maturity-discipline
// check-ts-maturity: enforce L1/L2/L3 layering over packages/ts via the
// typescript compiler API. File suffix decides the level:
//   unsuffixed .ts  = L1 contract: shapes only (interfaces, type aliases,
//                    enums, consts, imports). No function bodies, no classes
//                    with methods (a pure-data class with zero methods is OK).
//   *.flow.ts      = L2 wiring: no `TODO (L3)` / `(L3)` / `L3-GAP` tokens in
//                    comments or string literals; no IO (fs/net/child_process
//                    imports or calls).
//   *.run.ts       = L3 stubs: every function has a `// TODO (L3)` marker
//                    (leading trivia or same line) and a trivial body (a
//                    single return of a literal / empty collection / new
//                    Error(literal); no calls, if, loops, try).
// Skips node_modules, dist, .venv, tests, generated, *.d.ts.
// Findings print as `path:line: <rule>: <detail>`; exit 1 when any.
"use strict";

const fs = require("fs");
const path = require("path");
const ts = require("typescript");

const ROOT = path.resolve(__dirname, "..");
const TS_ROOTS = [
  path.join(ROOT, "packages/ts/core/src"),
  path.join(ROOT, "packages/ts/e2b-compat/src"),
];
const SKIP_DIRS = new Set(["node_modules", "dist", ".venv", "tests", "generated"]);
const TOKEN_RE = /TODO \(L3\)|\(L3\)|L3-GAP/;
const IO_MODULES = new Set(["fs", "node:fs", "net", "node:net", "child_process", "node:child_process"]);

const findings = [];

function walk(dir) {
  let out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      out = out.concat(walk(path.join(dir, entry.name)));
    } else if (entry.name.endsWith(".ts") && !entry.name.endsWith(".d.ts")) {
      out.push(path.join(dir, entry.name));
    }
  }
  return out;
}

function rel(file) {
  return path.relative(ROOT, file).split(path.sep).join("/");
}

function add(file, line, rule, detail) {
  findings.push(`${rel(file)}:${line}: ${rule}: ${detail}`);
}

function forEachNode(node, visit) {
  visit(node);
  ts.forEachChild(node, (child) => forEachNode(child, visit));
}

function isFunctionLike(node) {
  return (
    ts.isFunctionDeclaration(node) ||
    ts.isMethodDeclaration(node) ||
    ts.isGetAccessorDeclaration(node) ||
    ts.isSetAccessorDeclaration(node) ||
    ts.isConstructorDeclaration(node) ||
    ts.isArrowFunction(node) ||
    ts.isFunctionExpression(node)
  );
}

function functionName(node) {
  if (node.name && ts.isIdentifier(node.name)) return node.name.text;
  return "<anonymous>";
}

function hasBody(node) {
  return node.body !== undefined && node.body !== null;
}

function tokenInLeadingTrivia(source, node) {
  // getFullText() includes the comment trivia attached to the node
  if (node.getFullText && TOKEN_RE.test(node.getFullText())) return true;
  // marker on the same line as the body/arrow
  const start = node.getStart(source);
  const lineStart = source.text.lastIndexOf("\n", start - 1) + 1;
  let lineEnd = source.text.indexOf("\n", start);
  if (lineEnd === -1) lineEnd = source.text.length;
  return TOKEN_RE.test(source.text.slice(lineStart, lineEnd));
}

function isVoidSilencer(s) {
  return ts.isExpressionStatement(s) && ts.isVoidExpression(s.expression);
}

function bodyIsTrivial(node, source) {
  const body = node.body;
  if (!body) return { ok: true, why: "" };
  if (!ts.isBlock(body)) {
    // expression-bodied arrow: allow a single literal expression
    return { ok: isTrivialExpression(body), why: "non-trivial expression body" };
  }
  // void-silencer statements (void x;) are allowed before the terminal
  const stmts = body.statements.filter(
    (s) => s.kind !== ts.SyntaxKind.EmptyStatement && !isVoidSilencer(s),
  );
  if (stmts.length === 0) return { ok: true, why: "" };
  if (stmts.length > 1) return { ok: false, why: `${stmts.length} statements` };
  const s = stmts[0];
  if (ts.isReturnStatement(s) && (!s.expression || isTrivialExpression(s.expression))) {
    return { ok: true, why: "" };
  }
  if (s.kind === ts.SyntaxKind.ThrowStatement) {
    const expr = s.expression;
    if (
      expr &&
      ts.isNewExpression(expr) &&
      (!expr.arguments ||
        expr.arguments.every(
          (a) =>
            ts.isStringLiteral(a) ||
            ts.isNumericLiteral(a) ||
            ts.isIdentifier(a) ||
            ts.isBinaryExpression(a) === false && isTemplateLiteralish(a),
        ))
    ) {
      return { ok: true, why: "" };
    }
    if (expr && ts.isCallExpression(expr)) {
      // throw _missing("...") helper: callee identifier + literal-only args
      if (
        ts.isIdentifier(expr.expression) &&
        expr.arguments.every((a) => ts.isStringLiteral(a))
      ) {
        return { ok: true, why: "" };
      }
    }
    return { ok: false, why: "throw of non-literal" };
  }
  return { ok: false, why: `statement kind ${ts.SyntaxKind[s.kind]}` };
}

function isTemplateLiteralish(a) {
  return ts.isNoSubstitutionTemplateLiteral(a);
}

function isTrivialExpression(e) {
  if (ts.isStringLiteral(e) || ts.isNumericLiteral(e) || ts.isBooleanLiteral(e)) return true;
  if (e.kind === ts.SyntaxKind.NullKeyword || e.kind === ts.SyntaxKind.ThisKeyword) return true;
  if (ts.isIdentifier(e)) return true;
  if (ts.isArrayLiteralExpression(e) || ts.isObjectLiteralExpression(e)) {
    return e.elements.every(isTrivialExpression);
  }
  if (ts.isNewExpression(e)) {
    return (!e.arguments || e.arguments.every((a) => ts.isStringLiteral(a) || ts.isNumericLiteral(a) || ts.isIdentifier(a)));
  }
  return false;
}

function checkIO(source, node, file) {
  // import specifiers
  if (ts.isImportDeclaration(node) || ts.isCallExpression(node)) {
    const mod =
      ts.isImportDeclaration(node) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)
        ? node.moduleSpecifier.text
        : null;
    if (mod && (IO_MODULES.has(mod) || [...IO_MODULES].some((m) => mod.startsWith(m + "/")))) {
      add(file, node.getStart(source), "L2 live IO", `import of ${mod}`);
    }
  }
  if (ts.isCallExpression(node)) {
    const callee = node.expression;
    const text = callee.getText ? callee.getText(source) : "";
    if (ts.isIdentifier(callee)) {
      if (IO_FN_IDENTIFIERS.has(callee.text)) {
        add(file, node.getStart(source), "L2 live IO", `${callee.text}() call`);
      }
    } else if (ts.isPropertyAccessExpression(callee)) {
      if (IO_FN_PROPS.has(text)) {
        add(file, node.getStart(source), "L2 live IO", `${text}() call`);
      }
    }
    if (callee.kind === ts.SyntaxKind.ImportKeyword) {
      add(file, node.getStart(source), "L2 live IO", "dynamic import()");
    }
  }
}

const IO_FN_IDENTIFIERS = new Set([
  "readFile", "writeFile", "appendFile", "readdir", "mkdir", "rmdir", "unlink",
  "stat", "open", "spawn", "execFile", "execSync", "spawnSync", "fork",
  "createConnection", "createServer", "createSocket",
]);
const IO_FN_PROPS = new Set([
  "fs.readFile", "fs.writeFile", "fs.appendFile", "fs.readdir", "fs.mkdir",
  "fs.rmdir", "fs.unlink", "fs.stat", "fs.open", "net.createConnection",
  "net.createServer", "net.createSocket", "child_process.spawn",
  "child_process.execFile", "child_process.exec", "child_process.fork",
  "fs.promises.readFile", "fs.promises.writeFile", "fs.createReadStream",
  "fs.createWriteStream", "require",
]);

function checkCommentsAndStrings(source, file) {
  // comment trivia: raw-text scan (trivia is not walked by forEachChild)
  const text = source.text;
  const commentRe = /\/\*[\s\S]*?\*\/|\/\/[^\n]*/g;
  let m;
  while ((m = commentRe.exec(text)) !== null) {
    if (TOKEN_RE.test(m[0])) {
      const line = text.slice(0, m.index).split("\n").length;
      add(file, line, "L2 token", "TODO (L3)/(L3)/L3-GAP in comment");
    }
  }
  forEachNode(source, (node) => {
    if (ts.isStringLiteral(node) && TOKEN_RE.test(node.text)) {
      const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
      add(file, line, "L2 token", "(L3) token in string literal");
    }
    if (ts.isNoSubstitutionTemplateLiteral(node) && TOKEN_RE.test(node.text)) {
      const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
      add(file, line, "L2 token", "(L3) token in template literal");
    }
    if (ts.isTemplateExpression(node)) {
      const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
      const spans = [node.head.text, ...node.templateSpans.map((s) => s.literal.text)].join("");
      if (TOKEN_RE.test(spans)) add(file, line, "L2 token", "(L3) token in template literal");
    }
  });
}

function checkFile(file) {
  const text = fs.readFileSync(file, "utf8");
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2022, true);
  const name = path.basename(file);
  const isRun = /\.run\.ts$/.test(name);
  const isFlow = /\.flow\.ts$/.test(name);

  if (isFlow) {
    checkCommentsAndStrings(source, file);
    forEachNode(source, (node) => checkIO(source, node, file));
    return;
  }

  if (isRun) {
    forEachNode(source, (node) => {
      if (isFunctionLike(node) && hasBody(node)) {
        const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
        if (!tokenInLeadingTrivia(source, node)) {
          add(file, line, "L3 marker", `function ${functionName(node)} lacks TODO (L3) marker`);
        }
        const trivial = bodyIsTrivial(node, source);
        if (!trivial.ok) {
          add(file, line, "L3 trivial body", `function ${functionName(node)}: ${trivial.why}`);
        }
      }
    });
    return;
  }

  // L1: unsuffixed contract file
  forEachNode(source, (node) => {
    if (ts.isFunctionDeclaration(node) && hasBody(node)) {
      add(file, node.getStart(source), "L1 shapes only", `function ${functionName(node)} has a body`);
    }
    if (ts.isClassDeclaration(node) && node.name) {
      const hasMethods = node.members.some(
        (m) => ts.isMethodDeclaration(m) || ts.isGetAccessorDeclaration(m) || ts.isSetAccessorDeclaration(m) || ts.isConstructorDeclaration(m),
      );
      if (hasMethods) {
        add(file, node.getStart(source), "L1 shapes only", `class ${node.name.text} has methods`);
      }
    }
  });
}

function main() {
  const files = TS_ROOTS.flatMap((root) => (fs.existsSync(root) ? walk(root) : []))
    .sort();
  let runs = 0;
  for (const file of files) {
    if (/\.run\.ts$/.test(path.basename(file))) runs += 1;
    checkFile(file);
  }
  for (const f of findings) console.log(f);
  if (findings.length > 0) {
    console.log(`check-ts-maturity: FAIL (${findings.length} findings, ${files.length} files)`);
    process.exit(1);
  }
  console.log(`check-ts-maturity: PASS (${files.length} files, ${runs} .run stubs)`);
}

main();

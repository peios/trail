// Keep template markup intact: its whitespace controls rendered text. Format
// the embedded JavaScript with the same pinned formatter as standalone code.
import fs from "node:fs/promises";
import path from "node:path";
const prettier = await import(process.env.TRAIL_PRETTIER || "prettier");

const write = process.argv.includes("--write");
const options = JSON.parse(await fs.readFile(".prettierrc.json", "utf8"));

async function javascript(source) {
    // Braces avoid formatter-induced multiline unbraced clauses. Prettier's
    // parser API is internal, so the exact formatter version is locked.
    const { ast } = await prettier.__debug.parse(source, { parser: "babel" });
    const edits = [];
    function visit(node) {
        if (!node || typeof node !== "object") {
            return;
        }
        const bodies =
            node.type === "IfStatement"
                ? [node.consequent, node.alternate?.type === "IfStatement" ? null : node.alternate]
                : [
                        "ForStatement",
                        "ForOfStatement",
                        "ForInStatement",
                        "WhileStatement",
                        "DoWhileStatement",
                    ].includes(node.type)
                  ? [node.body]
                  : [];
        for (const body of bodies) {
            if (!body || body.type === "BlockStatement") {
                continue;
            }
            edits.push([body.start, "{ "], [body.end, " }"]);
        }
        for (const [key, value] of Object.entries(node)) {
            if (["tokens", "comments", "loc"].includes(key)) {
                continue;
            }
            if (Array.isArray(value)) {
                value.forEach(visit);
            } else if (value && typeof value === "object") {
                visit(value);
            }
        }
    }
    visit(ast);
    for (const [offset, text] of edits.sort((a, b) => b[0] - a[0])) {
        source = source.slice(0, offset) + text + source.slice(offset);
    }
    return prettier.format(source, { ...options, parser: "babel" });
}

async function format(file) {
    const source = await fs.readFile(file, "utf8");
    let result;
    if (file.endsWith(".html")) {
        result = source;
        const scripts = [...source.matchAll(/<script>([\s\S]*?)<\/script>/g)];
        for (const match of scripts.reverse()) {
            // This is the only template expression inside an inline script.
            const body = match[1].replace("{{ build_config }}", "TRAIL_BUILD_CONFIG");
            const formatted = (await javascript(body)).replace(
                "TRAIL_BUILD_CONFIG",
                "{{ build_config }}",
            );
            result =
                result.slice(0, match.index) +
                "<script>\n" +
                formatted +
                "</script>" +
                result.slice(match.index + match[0].length);
        }
    } else if (/\.(mjs|js)$/.test(file)) {
        result = await javascript(source);
    } else {
        result = await prettier.format(source, { ...options, filepath: file });
    }
    if (result !== source) {
        if (write) {
            await fs.writeFile(file, result);
        } else {
            console.error(`${file}: formatting differs; run npm run format`);
            process.exitCode = 1;
        }
    }
}

async function walk(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
        const file = path.join(directory, entry.name);
        if (entry.isDirectory()) {
            if (!["fonts", "vendor"].includes(entry.name)) {
                await walk(file);
            }
        } else if (/\.(js|mjs|css|html|yml|yaml)$/.test(file)) {
            await format(file);
        }
    }
}

for (const directory of ["theme", "tools", ".github"]) {
    await walk(directory);
}
for (const file of ["package.json", "package-lock.json", ".prettierrc.json"]) {
    await format(file);
}

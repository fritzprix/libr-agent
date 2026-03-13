const fs = require('fs');

const path = 'src/lib/backend/messages.test.ts';
let code = fs.readFileSync(path, 'utf8');

// The issue was `describe('getMessagesPageForSession', () => { ... }); });` -> extra bracket closed the parent describe block.
// I will just remove the extra `});` before `describe('upsertMessages', () => {`
code = code.replace(/    \}\);\n  \}\);\n\}\);\n\n  describe\('upsertMessages', \(\) => \{/g, "    });\n  });\n\n  describe('upsertMessages', () => {");
code += "});\n";
fs.writeFileSync(path, code);

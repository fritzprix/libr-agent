const { execSync } = require('child_process');

try {
  let latestTag;
  try {
    latestTag = execSync('git describe --tags --abbrev=0').toString().trim();
  } catch (e) {
    console.log('No tags found, listing all commits.');
    latestTag = null;
  }

  const range = latestTag ? `${latestTag}..HEAD` : 'HEAD';
  console.log(`\n>>> Analyzing changes for range: ${range}\n`);
  
  const log = execSync(`git log ${range} --pretty=format:"%s|%h|%an"`).toString();
  const commits = log.split('\n').filter(Boolean);
  
  const categories = {
    feat: [],
    fix: [],
    refactor: [],
    perf: [],
    docs: [],
    chore: [],
    test: [],
    other: []
  };

  commits.forEach(line => {
    const parts = line.split('|');
    if (parts.length < 2) return;
    
    const msg = parts[0];
    const hash = parts[1];
    
    // Regex for Conventional Commits: type(scope): description
    const match = msg.match(/^(\w+)(?:\(.*\))?!?: (.*)$/);
    if (match) {
      const type = match[1].toLowerCase();
      const desc = match[2];
      const entry = `- ${desc} (${hash})`; // excluding author to keep it clean
      
      if (categories[type]) {
        categories[type].push(entry);
      } else if (type === 'documentation') {
         categories.docs.push(entry);
      } else {
        categories.other.push(entry);
      }
    } else {
      categories.other.push(`- ${msg} (${hash})`);
    }
  });

  console.log('--- CHANGELOG DRAFT START ---');
  
  if (categories.feat.length) {
    console.log('\n#### 🚀 Features');
    categories.feat.forEach(c => console.log(c));
  }
  
  if (categories.fix.length) {
    console.log('\n#### 🐛 Bug Fixes');
    categories.fix.forEach(c => console.log(c));
  }
  
  if (categories.refactor.length) {
    console.log('\n#### 🔧 Refactoring & Improvements');
    categories.refactor.forEach(c => console.log(c));
  }

  if (categories.perf.length) {
    console.log('\n#### ⚡ Performance');
    categories.perf.forEach(c => console.log(c));
  }

  if (categories.docs.length) {
    console.log('\n#### 📝 Documentation');
    categories.docs.forEach(c => console.log(c));
  }
  
  if (categories.other.length > 0) {
    console.log('\n#### 🧩 Other Changes');
    categories.other.forEach(c => console.log(c));
  }

  console.log('\n--- CHANGELOG DRAFT END ---\n');
  console.log('Copy the sections above into CHANGELOG.md under a new [Unreleased] or version header.');

} catch (e) {
  console.error("Error generating changelog:", e.message);
  process.exit(1);
}

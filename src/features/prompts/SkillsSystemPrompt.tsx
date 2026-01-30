import { useCallback, useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import { useSystemPrompt } from '@/context/SystemPromptContext';
import { useSkills } from '@/context/SkillsContext';

const logger = getLogger('SkillsSystemPrompt');

// Escape XML special characters to prevent injection
function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

export function SkillsSystemPrompt() {
  const { register, unregister } = useSystemPrompt();
  const { skills } = useSkills();

  const buildPrompt = useCallback(async () => {
    if (skills.length === 0) {
      return '';
    }

    const skillsXml = skills
      .map(
        (skill) => `  <skill>
    <name>${escapeXml(skill.name)}</name>
    <description>${escapeXml(skill.description)}</description>
    <location>${escapeXml(skill.path)}</location>
  </skill>`,
      )
      .join('\n');

    return `<available_skills>
${skillsXml}
</available_skills>
`;
  }, [skills]);

  useEffect(() => {
    if (skills.length === 0) {
      return;
    }

    const id = register('agent-skills', buildPrompt, 2); // Priority 2
    logger.debug('Registered skills system prompt', {
      promptId: id,
      count: skills.length,
    });

    return () => {
      unregister(id);
      logger.debug('Unregistered skills system prompt', { promptId: id });
    };
  }, [buildPrompt, register, unregister, skills.length]);

  return null;
}

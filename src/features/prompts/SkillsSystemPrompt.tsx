import { useCallback, useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import { useSystemPrompt } from '@/context/SystemPromptContext';
import { useSkills } from '@/context/SkillsContext';

const logger = getLogger('SkillsSystemPrompt');

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
    <name>${skill.name}</name>
    <description>${skill.description}</description>
    <location>${skill.path}</location>
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

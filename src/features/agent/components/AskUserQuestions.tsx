import type { UserQuestion } from '@/models/agent-ipc';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

/** What the user has entered so far for one question. */
export interface QuestionDraft {
  selected: string[];
  otherChecked: boolean;
  /** Free-text answer, or the "Other" text when `otherChecked`. */
  text: string;
}

export type AskUserAnswers = Record<string, string | string[]>;

const EMPTY_DRAFT: QuestionDraft = {
  selected: [],
  otherChecked: false,
  text: '',
};

function draftFor(
  drafts: Record<string, QuestionDraft>,
  id: string,
): QuestionDraft {
  return drafts[id] ?? EMPTY_DRAFT;
}

function answerFor(
  question: UserQuestion,
  draft: QuestionDraft,
): string | string[] | null {
  const text = draft.text.trim();
  if (!question.options) {
    return text || null;
  }

  const usesOther = question.allowCustom && draft.otherChecked;
  if (usesOther && !text) {
    return null;
  }
  if (question.multiSelect) {
    const values = usesOther ? [...draft.selected, text] : draft.selected;
    return values.length > 0 ? values : null;
  }
  return usesOther ? text : (draft.selected[0] ?? null);
}

/** The submit payload, or `null` while any question is unanswered. */
export function buildAskUserAnswers(
  questions: UserQuestion[],
  drafts: Record<string, QuestionDraft>,
): AskUserAnswers | null {
  const answers: AskUserAnswers = {};
  for (const question of questions) {
    const answer = answerFor(question, draftFor(drafts, question.id));
    if (answer === null) {
      return null;
    }
    answers[question.id] = answer;
  }
  return answers;
}

function toggle(values: string[], value: string): string[] {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}

interface QuestionFieldProps {
  question: UserQuestion;
  draft: QuestionDraft;
  disabled: boolean;
  onChange: (id: string, draft: QuestionDraft) => void;
}

function QuestionField({
  question,
  draft,
  disabled,
  onChange,
}: QuestionFieldProps) {
  const update = (patch: Partial<QuestionDraft>) =>
    onChange(question.id, { ...draft, ...patch });
  const inputType = question.multiSelect ? 'checkbox' : 'radio';
  const groupName = `ask-user-${question.id}`;

  return (
    <fieldset className="space-y-2" disabled={disabled}>
      <legend className="text-sm font-medium">{question.question}</legend>
      {question.description ? (
        <p className="text-muted-foreground text-xs whitespace-pre-wrap">
          {question.description}
        </p>
      ) : null}
      {question.options ? (
        <div className="space-y-1">
          {question.options.map((option, index) => (
            <label
              key={`${index}-${option}`}
              className="flex items-center gap-2 text-sm"
            >
              <input
                type={inputType}
                name={groupName}
                checked={draft.selected.includes(option)}
                onChange={() =>
                  update(
                    question.multiSelect
                      ? { selected: toggle(draft.selected, option) }
                      : { selected: [option], otherChecked: false },
                  )
                }
              />
              {option}
            </label>
          ))}
          {question.allowCustom ? (
            <>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type={inputType}
                  name={groupName}
                  checked={draft.otherChecked}
                  onChange={() =>
                    update(
                      question.multiSelect
                        ? { otherChecked: !draft.otherChecked }
                        : { otherChecked: true, selected: [] },
                    )
                  }
                />
                Other
              </label>
              {draft.otherChecked ? (
                <Input
                  aria-label={`${question.question} (other)`}
                  value={draft.text}
                  onChange={(event) => update({ text: event.target.value })}
                />
              ) : null}
            </>
          ) : null}
        </div>
      ) : (
        <Textarea
          aria-label={question.question}
          value={draft.text}
          onChange={(event) => update({ text: event.target.value })}
          rows={3}
        />
      )}
    </fieldset>
  );
}

export interface AskUserQuestionsProps {
  questions: UserQuestion[];
  drafts: Record<string, QuestionDraft>;
  disabled: boolean;
  onChange: (id: string, draft: QuestionDraft) => void;
}

/** One section per `ui__askUser` question (#2547). */
export function AskUserQuestions({
  questions,
  drafts,
  disabled,
  onChange,
}: AskUserQuestionsProps) {
  return (
    <div className="max-h-96 space-y-5 overflow-y-auto">
      {questions.map((question) => (
        <QuestionField
          key={question.id}
          question={question}
          draft={draftFor(drafts, question.id)}
          disabled={disabled}
          onChange={onChange}
        />
      ))}
    </div>
  );
}

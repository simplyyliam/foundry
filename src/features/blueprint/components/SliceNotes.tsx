import { useState } from "react";
import { Check } from "lucide-react";
import { Input } from "@/components/ui/input";
import type { OpenQuestion, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { InlineAdd } from "./InlineAdd";
import { ActionError } from "./Notices";

interface SliceNotesProps {
  slice: SliceView;
  actions: PlanActions;
}

export function SliceDecisions({ slice, actions }: SliceNotesProps) {
  const { busy, error, run } = useAction();
  return (
    <div className="flex flex-col gap-2">
      {slice.decisions.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">None yet.</p>
      ) : (
        <ul className="flex list-disc flex-col gap-1 pl-5 text-[13px] leading-snug marker:text-muted-foreground">
          {slice.decisions.map((decision, index) => (
            <li key={`${index}:${decision}`}>{decision}</li>
          ))}
        </ul>
      )}
      <InlineAdd
        label="Add decision"
        placeholder="Add a decision"
        busy={busy}
        onSubmit={async (text) => (await run(() => actions.addDecision(slice.id, text))).ok}
      />
      <ActionError error={error} />
    </div>
  );
}

/** Open questions: add one, or resolve one with an answer (kept as a decision). */
export function SliceQuestions({ slice, actions }: SliceNotesProps) {
  const { busy, error, run } = useAction();
  return (
    <div className="flex flex-col gap-2">
      {slice.openQuestions.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">No open questions.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {slice.openQuestions.map((question) => (
            <QuestionItem key={question.id} sliceId={slice.id} question={question} actions={actions} />
          ))}
        </ul>
      )}
      <InlineAdd
        label="Add open question"
        placeholder="Ask a question"
        busy={busy}
        onSubmit={async (text) => (await run(() => actions.addOpenQuestion(slice.id, text))).ok}
      />
      <ActionError error={error} />
    </div>
  );
}

function QuestionItem({ sliceId, question, actions }: { sliceId: string; question: OpenQuestion; actions: PlanActions }) {
  const [resolving, setResolving] = useState(false);
  const [answer, setAnswer] = useState("");
  const { busy, error, run, clear } = useAction();

  return (
    <li className="flex flex-col gap-1.5 rounded-xl bg-muted/60 px-3 py-2">
      <div className="flex items-start gap-2">
        <p className="min-w-0 flex-1 text-[13px] leading-snug">{question.text}</p>
        {!resolving && (
          <PressButton variant="ghost" size="xs" className="-my-0.5 shrink-0" onClick={() => setResolving(true)}>
            Resolve
          </PressButton>
        )}
      </div>
      {resolving && (
        <ResolveForm
          answer={answer}
          busy={busy}
          onChange={setAnswer}
          onCancel={() => {
            setResolving(false);
            clear();
          }}
          onSubmit={async () => {
            await run(() => actions.resolveOpenQuestion(sliceId, question.id, answer.trim()));
            // On success the question is gone from the plan, and this item with it.
          }}
        />
      )}
      <ActionError error={error} />
    </li>
  );
}

interface ResolveFormProps {
  answer: string;
  busy: boolean;
  onChange: (answer: string) => void;
  onCancel: () => void;
  onSubmit: () => Promise<void>;
}

function ResolveForm({ answer, busy, onChange, onCancel, onSubmit }: ResolveFormProps) {
  return (
    <form
      className="flex items-center gap-1.5"
      onSubmit={(event) => {
        event.preventDefault();
        if (answer.trim() && !busy) void onSubmit();
      }}
    >
      <Input
        autoFocus
        value={answer}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.stopPropagation();
            onCancel();
          }
        }}
        placeholder="The answer"
        aria-label="Resolution"
        className="h-8 bg-background text-[13px]"
      />
      <PressButton pressScale={0.92} type="submit" size="icon-sm" aria-label="Resolve question" disabled={!answer.trim() || busy}>
        <Check aria-hidden />
      </PressButton>
    </form>
  );
}

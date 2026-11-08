// A one-line input that adds what was typed: by Enter on a computer, by its
// Add button anywhere.
//
// Japanese input on a computer confirms the conversion with Enter, which must
// not add the line yet. On phones Enter never adds: keyboards (Gboard and
// others) send it to confirm a conversion too, and keep even plain words
// "composing", so it cannot be told apart from "add" — only the button adds.

import type { ReactNode, Ref } from "react";
import { isMobile } from "../lib/mobile";
import { Icon } from "./Icon";

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** Called with the trimmed text, when there is some. */
  onAdd: (text: string) => void;
  placeholder?: string;
  className?: string;
  inputClassName?: string;
  /** Label of the button (an icon only when not given). */
  buttonLabel?: string;
  /** The button stays visible even when nothing is typed (phones: so it can be found). */
  alwaysShowButton?: boolean;
  inputRef?: Ref<HTMLInputElement>;
  /** Before the input (an icon, a time picker…). */
  before?: ReactNode;
  /** Between the input and the button. */
  after?: ReactNode;
}

export function AddInput({ value, onChange, onAdd, placeholder, className, inputClassName, buttonLabel, alwaysShowButton, inputRef, before, after }: Props) {
  const submit = () => {
    const text = value.trim();
    if (text) onAdd(text);
  };
  return (
    <form
      className={`add-input ${className ?? ""}`}
      onSubmit={(e) => {
        e.preventDefault();
        // Phones: Enter (the form's implicit submission) does nothing; the button adds.
        if (!isMobile) submit();
      }}
    >
      {before}
      <input
        ref={inputRef}
        className={inputClassName ?? "input"}
        placeholder={placeholder}
        value={value}
        enterKeyHint={isMobile ? "enter" : "done"}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          // On a computer, Enter while converting Japanese only confirms the text.
          if (e.key === "Enter" && e.nativeEvent.isComposing && !isMobile) e.preventDefault();
        }}
      />
      {after}
      {(value.trim() || alwaysShowButton || isMobile) && (
        <button
          className={`btn add-input-btn${value.trim() ? " primary" : ""}`}
          type={isMobile ? "button" : "submit"}
          disabled={!value.trim()}
          title={buttonLabel}
          onClick={isMobile ? submit : undefined}
        >
          {buttonLabel && !isMobile ? buttonLabel : <Icon name="plus" />}
        </button>
      )}
    </form>
  );
}

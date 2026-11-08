// What follows the pointer while dragging: a small chip in the app's style,
// instead of the system's faded copy of the element.

/** Sets the drag image of `e` to a chip saying `label`. */
export function dragChip(e: { dataTransfer: DataTransfer }, label: string) {
  const chip = document.createElement("div");
  chip.className = "drag-chip";
  chip.textContent = label.length > 40 ? `${label.slice(0, 40)}…` : label;
  document.body.appendChild(chip);
  e.dataTransfer.setDragImage(chip, 14, 14);
  // The image is taken now; the element can go.
  requestAnimationFrame(() => chip.remove());
}

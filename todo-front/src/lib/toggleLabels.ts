import { Label } from "../types/todos"

export const toggleLabels = (labels: Label[], target: Label) =>
  labels.find(({ id }) => id === target.id)
    ? labels.filter(({ id }) => id !== target.id)
    : [...labels, target]

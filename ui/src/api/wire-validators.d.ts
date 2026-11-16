declare module 'virtual:wire-validators' {
  const validators: Record<string, (value: unknown) => boolean>
  export = validators
}

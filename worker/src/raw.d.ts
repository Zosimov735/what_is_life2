/**
 * The bundler's raw-import form, declared for the type checker.
 *
 * Authored content is imported as its bytes rather than as parsed data, because
 * the bytes are what the content hash is over. The bundler inlines the file's
 * text at build time; nothing is fetched.
 */
declare module '*?raw' {
  const text: string;
  export default text;
}

/** A load every caller shares until it fails, after which the next caller runs it again. */
export class SharedLoad<T> {
  private promise: Promise<T> | undefined;

  get(load: () => Promise<T>): Promise<T> {
    this.promise ??= load().catch((error) => {
      this.promise = undefined;
      throw error;
    });

    return this.promise;
  }
}

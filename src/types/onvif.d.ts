declare module "onvif" {
  class Cam {
    constructor(
      options: {
        hostname: string;
        port: number;
        username: string;
        password: string;
        timeout?: number;
        preserveAddress?: boolean;
      },
      callback: (err: Error | null) => void,
    );

    getDeviceInformation(callback: (err: Error | null, info: any) => void): void;
    getProfiles(callback: (err: Error | null, profiles: any[]) => void): void;
    getStreamUri(
      options: { protocol: string; profileToken: string },
      callback: (err: Error | null, stream: any) => void,
    ): void;
    getSnapshotUri(
      options: { profileToken: string },
      callback: (err: Error | null, uri: any) => void,
    ): void;
  }

  class Discovery {
    static probe(
      options: { timeout?: number },
      callback: (err: Error | null, cams: any[]) => void,
    ): void;
  }

  const _default: {
    Cam: typeof Cam;
    Discovery: typeof Discovery;
  };

  export default _default;
}

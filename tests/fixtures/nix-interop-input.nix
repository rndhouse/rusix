# Local input-shaped object. No flake registry, fetch, or package build.
{
  functions.lazy = _: { good = 42; bad = throw "delayed external call failed"; };
  values.lazy = { good = 42; bad = throw "unselected opaque value failed"; };
  packages.example = builtins.derivation {
    name = "rusnix-external-1.0";
    pname = "rusnix-external";
    version = "1.0";
    system = "x86_64-linux";
    builder = "/never-built-by-rusnix";
  };
  nixosModules.example = { lib, ... }: {
    options.services.rusnixExternal.enable = lib.mkEnableOption "local example";
  };
  nixosModules.broken = { lib, ... }: {
    options.services.rusnixExternal.value = lib.mkOption {
      type = lib.types.int;
      default = throw "local external module failed";
    };
  };
  nixosModules.invalid = 42;
  overlays.example = final: prev: {
    rusnixOverlayHello = prev.hello;
  };
}

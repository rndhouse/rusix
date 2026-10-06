{
  compare = { graph, nixpkgs, opensslArgs ? {}, probe ? false }:
  let
    pkgs = import nixpkgs { system = "x86_64-linux"; config = {}; };
    openssl = pkgs.openssl.override opensslArgs;
    curl = pkgs.curlMinimal.override { inherit openssl; };
    git = pkgs.git.override { inherit openssl; };
    project = p: {
      derivationPath = p.drvPath;
      recipe = builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
      outputs = pkgs.lib.genAttrs p.outputs (o: p.${o}.outPath);
    };
    projection = g: builtins.mapAttrs (_: project) g;
    matches = { upstream = projection { inherit openssl curl git; }; candidate = projection graph; };
    opensslInputs = builtins.filter (p: (p.pname or "") == "openssl") graph.git.buildInputs;
  in matches // pkgs.lib.optionalAttrs probe {
    # Neither marker exists on pkgs.openssl. It must come from the supplied value.
    edges = {
      curlOpenSSL = graph.curl.openssl.rusnixAuthor;
      gitOpenSSL = (builtins.head opensslInputs).rusnixAuthor;
      sameOpenSSL = graph.curl.openssl.drvPath == graph.openssl.drvPath;
    };
  };
}

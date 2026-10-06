{
  compare = { graph, nixpkgs, opensslArgs ? {}, probe ? false, mariadbRelease ? "mariadb_1011" }:
  let
    pkgs = import nixpkgs { system = "x86_64-linux"; config = {}; };
    openssl = pkgs.openssl.override opensslArgs;
    curl = pkgs.curl.override { inherit openssl; };
    git = pkgs.git.override { inherit openssl; };
    mariadb = pkgs.${mariadbRelease}.override { inherit curl; };
    project = p: {
      derivationPath = p.drvPath;
      recipe = builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
      outputs = pkgs.lib.genAttrs p.outputs (o: p.${o}.outPath);
    };
    projection = g: builtins.mapAttrs (_: project) g // { mariadbClient = project g.mariadb.client; };
    matches = { upstream = projection { inherit openssl curl git mariadb; }; candidate = projection graph; };
    opensslInputs = builtins.filter (p: (p.pname or "") == "openssl") graph.git.buildInputs;
  in matches // pkgs.lib.optionalAttrs probe {
    # Neither marker exists on pkgs.openssl. It must come from the supplied value.
    edges = {
      curlOpenSSL = graph.curl.openssl.rusnixAuthor;
      gitOpenSSL = (builtins.head opensslInputs).rusnixAuthor;
      sameOpenSSL = graph.curl.openssl.drvPath == graph.openssl.drvPath;
      mariadbCurl = (builtins.head (builtins.filter (p: (p.pname or "") == "curl") graph.mariadb.buildInputs)).rusnixAuthor;
    };
  };
}

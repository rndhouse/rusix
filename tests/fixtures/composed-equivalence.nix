{
  compare = { graph, nixpkgs, opensslArgs ? {}, probe ? false, mariadbRelease ? "mariadb_1011", opensslRelease ? "openssl_3_3", curlArgs ? {}, mariadbArgs ? {}, nodes ? null, reverse ? false }:
  let
    pkgs = import nixpkgs { system = "x86_64-linux"; config = {}; };
    openssl = pkgs.${opensslRelease}.override opensslArgs;
    curl = pkgs.curl.override ({ inherit openssl; } // curlArgs);
    git = pkgs.git.override { inherit openssl; };
    mariadb = pkgs.${mariadbRelease}.override ({ inherit curl; } // mariadbArgs);
    project = p: {
      derivationPath = p.drvPath;
      recipe = builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
      outputs = pkgs.lib.genAttrs p.outputs (o: p.${o}.outPath);
    };
    projection = g: builtins.mapAttrs (_: project) (if nodes == null then g else pkgs.lib.getAttrs nodes g) // pkgs.lib.optionalAttrs (nodes == null || builtins.elem "mariadb" nodes) { mariadbClient = project g.mariadb.client; };
    matches = { upstream = projection { inherit openssl curl git mariadb; }; candidate = projection graph; };
    opensslInputs = builtins.filter (p: (p.pname or "") == "openssl") graph.git.buildInputs;
  in matches // pkgs.lib.optionalAttrs reverse {
    reverseEdges = {
      curlOpenSSL = (pkgs.curl.override { openssl = graph.openssl; }).openssl.rusnixAuthor;
      gitOpenSSL = (builtins.head (builtins.filter (p: (p.pname or "") == "openssl") (pkgs.git.override { openssl = graph.openssl; }).buildInputs)).rusnixAuthor;
      mariadbCurl = (builtins.head (builtins.filter (p: (p.pname or "") == "curl") (pkgs.mariadb.override { curl = graph.curl; }).buildInputs)).rusnixAuthor;
    };
    reverseConsumers = {
      curl = project (pkgs.curl.override { openssl = graph.openssl; });
      git = project (pkgs.git.override { openssl = graph.openssl; });
      mariadb = project (pkgs.mariadb.override { curl = graph.curl; });
    };
  } // pkgs.lib.optionalAttrs probe {
    # Neither marker exists on pkgs.openssl. It must come from the supplied value.
    edges = {
      curlOpenSSL = graph.curl.openssl.rusnixAuthor;
      gitOpenSSL = (builtins.head opensslInputs).rusnixAuthor;
      sameOpenSSL = graph.curl.openssl.drvPath == graph.openssl.drvPath;
      mariadbCurl = (builtins.head (builtins.filter (p: (p.pname or "") == "curl") graph.mariadb.buildInputs)).rusnixAuthor;
    };
  };
}

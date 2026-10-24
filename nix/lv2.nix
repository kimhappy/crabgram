{
  runCommand,
  lv2,
  inputs,
}:
runCommand "lv2-bundles" { } ''
  mkdir -p $out
  cp -r ${lv2}/lib/lv2/* $out/
  cp -r ${inputs.darkglass-lv2-extensions}/dg-* $out/
  cp -r ${inputs.kxstudio-lv2-extensions}/kx-* $out/
  chmod -R u+w $out

  sed -i '1i @prefix rdf:   <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .' \
      $out/dg-license.lv2/license.ttl

  sed -i '/^licns:uri/,/\.$/ s/rdfs:range rdf:Property/rdfs:range rdfs:Resource/' \
      $out/dg-license.lv2/license.ttl

  for overlay in blocked inactive; do
    printf '\ndgcs:%s\n    a rdf:Property , owl:ObjectProperty ;\n    rdfs:range dgcs:Overlay ;\n    rdfs:label "%s" .\n' \
        "$overlay" "$overlay" >> $out/dg-custom-styling.lv2/custom-styling.ttl
  done
''

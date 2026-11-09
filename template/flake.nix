{
  inputs.crabgram.url = "github:kimhappy/crabgram";

  outputs =
    { crabgram, ... }:
    {
      devShells = crabgram.devShells;
    };
}

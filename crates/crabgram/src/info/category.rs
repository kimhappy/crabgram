/// The LV2 plugin class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// `lv2:AmplifierPlugin`.
    Amplifier,
    /// `lv2:AnalyserPlugin`.
    Analyser,
    /// `lv2:ChorusPlugin`.
    Chorus,
    /// `lv2:CompressorPlugin`.
    Compressor,
    /// `lv2:DelayPlugin`.
    Delay,
    /// `lv2:DistortionPlugin`.
    Distortion,
    /// `lv2:DynamicsPlugin`.
    Dynamics,
    /// `lv2:EQPlugin`.
    Equalizer,
    /// `lv2:FilterPlugin`.
    Filter,
    /// `lv2:FlangerPlugin`.
    Flanger,
    /// `lv2:GatePlugin`.
    Gate,
    /// `lv2:GeneratorPlugin`.
    Generator,
    /// `lv2:LimiterPlugin`.
    Limiter,
    /// `lv2:MixerPlugin`.
    Mixer,
    /// `lv2:ModulatorPlugin`.
    Modulator,
    /// `lv2:PhaserPlugin`.
    Phaser,
    /// `lv2:PitchPlugin`.
    Pitch,
    /// `lv2:ReverbPlugin`.
    Reverb,
    /// `lv2:SimulatorPlugin`, for amplifier and cabinet simulations.
    Simulator,
    /// `lv2:SpatialPlugin`.
    Spatial,
    /// `lv2:UtilityPlugin`.
    Utility,
    /// `lv2:WaveshaperPlugin`.
    Waveshaper,
}

impl Category {
    /// The class as a Turtle prefixed name.
    ///
    /// ```
    /// assert_eq!(crabgram::Category::Modulator.curie(), "lv2:ModulatorPlugin");
    /// ```
    #[must_use]
    pub const fn curie(self) -> &'static str {
        match self {
            Self::Amplifier => "lv2:AmplifierPlugin",
            Self::Analyser => "lv2:AnalyserPlugin",
            Self::Chorus => "lv2:ChorusPlugin",
            Self::Compressor => "lv2:CompressorPlugin",
            Self::Delay => "lv2:DelayPlugin",
            Self::Distortion => "lv2:DistortionPlugin",
            Self::Dynamics => "lv2:DynamicsPlugin",
            Self::Equalizer => "lv2:EQPlugin",
            Self::Filter => "lv2:FilterPlugin",
            Self::Flanger => "lv2:FlangerPlugin",
            Self::Gate => "lv2:GatePlugin",
            Self::Generator => "lv2:GeneratorPlugin",
            Self::Limiter => "lv2:LimiterPlugin",
            Self::Mixer => "lv2:MixerPlugin",
            Self::Modulator => "lv2:ModulatorPlugin",
            Self::Phaser => "lv2:PhaserPlugin",
            Self::Pitch => "lv2:PitchPlugin",
            Self::Reverb => "lv2:ReverbPlugin",
            Self::Simulator => "lv2:SimulatorPlugin",
            Self::Spatial => "lv2:SpatialPlugin",
            Self::Utility => "lv2:UtilityPlugin",
            Self::Waveshaper => "lv2:WaveshaperPlugin",
        }
    }
}

import React from 'react';

interface ImageContentProps {
  image: {
    data?: string;
    source?: { data?: string; uri?: string };
    mimeType?: string;
  };
}

export const ImageContent: React.FC<ImageContentProps> = ({ image }) => {
  const imageSrc = image.data || image.source?.data || image.source?.uri;
  return imageSrc ? (
    <img
      src={imageSrc}
      alt="Tool output"
      className="max-w-full h-auto rounded-lg shadow-sm"
    />
  ) : null;
};

interface AudioContentProps {
  audio: {
    data?: string;
    mimeType?: string;
  };
}

export const AudioContent: React.FC<AudioContentProps> = ({ audio }) => {
  return audio.data ? (
    <audio controls className="w-full">
      <source src={audio.data} type={audio.mimeType} />
      Your browser does not support the audio element.
    </audio>
  ) : null;
};

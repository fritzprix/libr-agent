import React from "react";
import ChatContainer from "../ChatContainer";
import { MultiAgentOrchestrator } from "../orchestrators/MultiAgentOrchestrator";

/**
 * Example component demonstrating how to use the new peer architecture
 * for multi-agent chatting using composition.
 *
 * This component shows how MultiAgentOrchestrator is now composed as a child
 * of ChatContainer, extending Chat functionality through composition rather
 * than being toggled internally within Chat.
 *
 * When MultiAgentOrchestrator is enclosed within ChatContainer, multi-agent
 * mode is always active - no toggle logic is needed.
 */
const MultiAgentChat: React.FC = () => {
  return (
    <ChatContainer>
      <MultiAgentOrchestrator />
    </ChatContainer>
  );
};

export default MultiAgentChat;

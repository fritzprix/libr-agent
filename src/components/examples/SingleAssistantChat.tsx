import React from "react";
import ChatContainer from "../ChatContainer";
import Reflection from "../Reflection";

/**
 * Example component demonstrating how to use the new peer architecture
 * for single assistant chatting with optional agent mode features.
 *
 * This component shows how ChatContainer now manages the session state
 * and renders either StartSingleChatView or Chat as peer components,
 * rather than Chat containing StartSingleChatView as a child.
 */
const SingleAssistantChat: React.FC = () => {
  // In a real implementation, you might have context or state
  // to determine if agent mode (autonomous task execution) is enabled
  const isAgentMode = false; // Placeholder - would come from context

  return (
    <ChatContainer>
      {/*
        The Reflection component is a placeholder for supporting
        Autonomous Task execution in SingleAssistantChat.
        It would only be rendered when agent mode is active.
      */}
      {isAgentMode && <Reflection />}
    </ChatContainer>
  );
};

export default SingleAssistantChat;

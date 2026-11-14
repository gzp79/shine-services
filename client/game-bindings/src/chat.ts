import type { ChatCommand as ChatCommandType, ChatState as ChatStateType } from '../generated/postcard-bindings';
import { CHAT_MAX_TEXT_BYTES, CHAT_WINDOW } from '../generated/snapshots';
import { uuidToBytes, wireCodec } from './codec';

export type { ChatMessage } from '../generated/postcard-bindings';
export { CHAT_MAX_TEXT_BYTES, CHAT_WINDOW };

export type ChatState = ChatStateType;
export const ChatState = wireCodec('ChatState');

export type ChatCommand = ChatCommandType;

// Ids are uuid strings.
export const ChatCommand = {
    ...wireCodec('ChatCommand'),
    add: (text: string): ChatCommand => ({ tag: 'add', value: { text } }),
    remove: (id: string): ChatCommand => ({ tag: 'remove', value: { id: uuidToBytes(id) } }),
    replace: (id: string, text: string): ChatCommand => ({ tag: 'replace', value: { id: uuidToBytes(id), text } })
};

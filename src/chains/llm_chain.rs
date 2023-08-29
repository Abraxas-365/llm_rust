use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    chat_models::chat_model_trait::ChatTrait,
    errors::ApiError,
    schemas::{
        memory::BaseChatMessageHistory,
        messages::BaseMessage,
        prompt::{BasePromptValue, PromptData},
    },
};

use super::chain_trait::ChainTrait;

pub struct LLMChain<'a> {
    prompt: Box<dyn BasePromptValue>,
    header_prompts: Option<Vec<Box<dyn BaseMessage>>>,
    sandwich_prompts: Option<Vec<Box<dyn BaseMessage>>>,
    llm: Box<dyn ChatTrait>,
    pub memory: Option<&'a mut dyn BaseChatMessageHistory>,
}

impl<'a> LLMChain<'a> {
    pub fn new(prompt: Box<dyn BasePromptValue>, llm: Box<dyn ChatTrait>) -> Self {
        Self {
            prompt,
            llm,
            memory: None,
            header_prompts: None,
            sandwich_prompts: None,
        }
    }

    pub fn with_memory(mut self, memory: &'a mut dyn BaseChatMessageHistory) -> Self {
        self.memory = Some(memory);
        self
    }

    pub fn with_header_prompts(mut self, header_prompts: Vec<Box<dyn BaseMessage>>) -> Self {
        self.header_prompts = Some(header_prompts);
        self
    }

    pub fn sandwich_prompts(mut self, sandwich_prompts: Vec<Box<dyn BaseMessage>>) -> Self {
        self.sandwich_prompts = Some(sandwich_prompts);
        self
    }

    fn order_messages(
        &self,
        prompt_messages: Vec<Box<dyn BaseMessage>>,
    ) -> Vec<Vec<Box<dyn BaseMessage>>> {
        let mut all_messages: Vec<Vec<Box<dyn BaseMessage>>> = Vec::new();

        if let Some(header) = self.header_prompts.as_ref() {
            all_messages.push(header.clone());
        }

        all_messages.push(
            self.memory
                .as_ref()
                .map_or(Vec::new(), |memory| memory.messages()),
        );

        if let Some(sandwich) = self.sandwich_prompts.as_ref() {
            all_messages.push(sandwich.clone());
        }

        all_messages.push(prompt_messages);

        all_messages
    }

    async fn execute(
        &mut self,
        prompt_messages: Vec<Box<dyn BaseMessage>>,
    ) -> Result<String, ApiError> {
        let all_messages = self.order_messages(prompt_messages.clone());

        let ai_response = self.llm.generate(all_messages).await?;

        if let Some(memory) = self.memory.as_mut() {
            for message in &prompt_messages {
                log::debug!("message: {:?}", message.get_content());
                if message.get_type() != "system".to_string() {
                    log::debug!("Adding to memory: {:?}", message.get_content());
                    memory.add_message(message.clone());
                }
            }
            memory.add_message(Box::new(ai_response.clone()));
        }

        Ok(ai_response.get_content())
    }
}

#[async_trait]
impl<'a> ChainTrait<HashMap<String, String>> for LLMChain<'a> {
    async fn run(&mut self, inputs: HashMap<String, String>) -> Result<String, ApiError> {
        self.prompt.add_values(PromptData::HashMapData(inputs));
        let prompt_messages = self
            .prompt
            .to_chat_messages()
            .map_err(ApiError::PromptError)?;
        self.execute(prompt_messages).await
    }
}

#[async_trait]
impl<'a> ChainTrait<String> for LLMChain<'a> {
    async fn run(&mut self, inputs: String) -> Result<String, ApiError> {
        self.prompt.add_values(PromptData::VecData(vec![inputs]));
        let prompt_messages = self
            .prompt
            .to_chat_messages()
            .map_err(ApiError::PromptError)?;
        self.execute(prompt_messages).await
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        chains::llm_chain::LLMChain, chat_models::openai::chat_llm::ChatOpenAI,
        prompt::prompt::PromptTemplate,
    };

    use super::*;

    #[tokio::test]
    async fn test_llmchain_run_with_string() {
        let chat_openai = ChatOpenAI::default();
        let prompt_template = PromptTemplate::new("Hola mi nombre es {{name}}.");
        let mut llm_chain = LLMChain::new(Box::new(prompt_template), Box::new(chat_openai));
        let result = llm_chain.run("luis".to_string()).await;
        assert!(result.is_ok());
    }
}

use std::any::Any;
use std::error::Error;
use std::io;

use crate::regressor;
use crate::feature_buffer;
use crate::port_buffer;
use crate::model_instance;
use crate::graph;

use regressor::BlockTrait;

#[derive(PartialEq)]
pub enum Observe {
    Forward,
    Backward
}

pub struct BlockObserve {
    num_inputs: usize,
    input_offset: usize,
    observe: Observe,
    replace_backward_with: Option<f32>,
}


pub fn new_observe_block(bg: &mut graph::BlockGraph,
                        input: graph::BlockPtrOutput,
                        observe: Observe,
                        replace_backward_with: Option<f32>) 
                        -> Result<graph::BlockPtrOutput, Box<dyn Error>> {

    let num_inputs = bg.get_num_output_values(vec![&input]);
//    println!("Inputs: {} vec: {:?}", num_inputs, input);
    let block = Box::new(BlockObserve {
                         num_inputs: num_inputs as usize,
                         input_offset: usize::MAX,
                         observe: observe,
                         replace_backward_with: replace_backward_with});
    let mut block_outputs = bg.add_node(block, vec![input])?;
    assert_eq!(block_outputs.len(), 1);
    Ok(block_outputs.pop().unwrap())
}



impl BlockTrait for BlockObserve {
    // Warning: It does not confirm to regular clean-up after itself

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn get_block_type(&self) -> graph::BlockType {graph::BlockType::Observe}  

    fn get_num_output_slots(&self) -> usize {1} // It is a pass-through   

    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() == 0);
        // this means outputs on regular tapes
        return self.num_inputs
    }

    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        assert!(input.get_input_index() == 0);
        self.input_offset = offset;
    }

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize)  {
        assert!(output.get_output_index() == 0);
        assert_eq!(self.input_offset, offset); // this block type has special treatment
    }

    fn get_input_offset(&mut self, input: graph::InputSlot) -> Result<usize, Box<dyn Error>> {
        assert!(input.get_input_index() == 0);
        Ok(self.input_offset)
    }



    
    #[inline(always)]
    fn forward_backward(&mut self, 
                    further_blocks: &mut [Box<dyn BlockTrait>], 
                    fb: &feature_buffer::FeatureBuffer, 
                    pb: &mut port_buffer::PortBuffer, 
                    update:bool) {
        debug_assert!(self.input_offset != usize::MAX);

        // copy inputs to result
//        println!("Result block with Num inputs: {}", self.num_inputs);
        if self.observe == Observe::Forward {
            pb.observations.extend_from_slice(&pb.tape[self.input_offset .. (self.input_offset + self.num_inputs)]);
        }

        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
            next_regressor[0].forward_backward(further_blocks, fb, pb, update);
        }
        
        if self.observe == Observe::Backward {
            pb.observations.extend_from_slice(&pb.tape[self.input_offset .. (self.input_offset + self.num_inputs)]);
        }

        // replace inputs with whatever we wanted
        match self.replace_backward_with {
            Some(value) => pb.tape[self.input_offset..(self.input_offset + self.num_inputs)].fill(value),
            None => {},
        }

    }

    fn forward(&self, 
                     further_blocks: &[Box<dyn BlockTrait>], 
                     fb: &feature_buffer::FeatureBuffer,
                     pb: &mut port_buffer::PortBuffer
                     ) {
        debug_assert!(self.input_offset != usize::MAX);
        
        if self.observe == Observe::Forward {
            pb.observations.extend_from_slice(&pb.tape[self.input_offset .. (self.input_offset + self.num_inputs)]);
        }

        
        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at(1);
            next_regressor[0].forward(further_blocks, fb, pb);
        }


        if self.observe == Observe::Backward {
            pb.observations.extend_from_slice(&pb.tape[self.input_offset .. (self.input_offset + self.num_inputs)]);
        }

        // replace inputs with whatever we wanted
        match self.replace_backward_with {
            Some(value) => pb.tape[self.input_offset..(self.input_offset + self.num_inputs)].fill(value),
            None => {},
        }


    }

}





pub struct BlockConsts {
    pub output_offset: usize,
    consts: Vec<f32>,
    
}
/*
pub fn new_const_block(consts: Vec<f32>) -> Result<Box<dyn BlockTrait>, Box<dyn Error>> {
    Ok(Box::new(BlockConsts {   output_tape_index: -1,
                                consts: consts}))
}*/

pub fn new_const_block( bg: &mut graph::BlockGraph, 
                        consts: Vec<f32>) 
                        -> Result<graph::BlockPtrOutput, Box<dyn Error>> {
    let block = Box::new(BlockConsts {   output_offset: usize::MAX,
                                         consts: consts});
    let mut block_outputs = bg.add_node(block, vec![])?;
    assert_eq!(block_outputs.len(), 1);
    Ok(block_outputs.pop().unwrap())


}



impl BlockTrait for BlockConsts {
    // Warning: It does not confirm to regular clean-up after itself

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn get_num_output_slots(&self) -> usize {1}   

    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() == 0);
        self.consts.len() as usize
    }
    
    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        panic!("You cannnot set input_tape_index for BlockConsts");
    }

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize) {
        assert!(output.get_output_index() == 0, "Only supports a single output for BlockConsts");
        self.output_offset = offset;
    }

    #[inline(always)]
    fn forward_backward(&mut self, 
                    further_blocks: &mut [Box<dyn BlockTrait>], 
                    fb: &feature_buffer::FeatureBuffer, 
                    pb: &mut port_buffer::PortBuffer, 
                    update:bool) {
        debug_assert!(self.output_offset != usize::MAX);

        pb.tape[self.output_offset..(self.output_offset + self.consts.len())].copy_from_slice(&self.consts);

        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
            next_regressor[0].forward_backward(further_blocks, fb, pb, update);
        }
    }

    fn forward(&self, 
                     further_blocks: &[Box<dyn BlockTrait>], 
                     fb: &feature_buffer::FeatureBuffer,
                     pb: &mut port_buffer::PortBuffer, ) {

        debug_assert!(self.output_offset != usize::MAX);
        pb.tape[self.output_offset..(self.output_offset + self.consts.len())].copy_from_slice(&self.consts);

        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at(1);
            next_regressor[0].forward(further_blocks, fb, pb);
        }

    }

}


pub struct BlockCopy {    
    pub num_inputs: usize,
    pub input_offset: usize,
    pub output_offset: usize,
}


pub fn new_copy_block(bg: &mut graph::BlockGraph,
                       input: graph::BlockPtrOutput
                       ) -> Result<Vec<graph::BlockPtrOutput>, Box<dyn Error>> {
    let num_inputs = bg.get_num_output_values(vec![&input]);
    assert!(num_inputs != 0);

    let mut block = Box::new(BlockCopy {
        output_offset: usize::MAX,
        input_offset: usize::MAX,
        num_inputs: num_inputs as usize,
    });
    let block_outputs = bg.add_node(block, vec![input])?;
    assert_eq!(block_outputs.len(), 2);
    Ok(block_outputs)
}





impl BlockTrait for BlockCopy

 {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn get_block_type(&self) -> graph::BlockType {graph::BlockType::Copy}  

    fn allocate_and_init_weights(&mut self, mi: &model_instance::ModelInstance) {
    }

    fn get_num_output_slots(&self) -> usize {2}   

    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() <= 1);
        self.num_inputs
    }

    fn get_input_offset(&mut self, input: graph::InputSlot) -> Result<usize, Box<dyn Error>> {
        assert!(input.get_input_index() == 0);
        Ok(self.input_offset)
    }


    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        assert!(input.get_input_index() == 0);
        self.input_offset = offset;
    }

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize) {
        if output.get_output_index() == 0 {
            assert!(self.input_offset == offset)
        } else 
        if output.get_output_index() == 1 {
            self.output_offset = offset;
        } else {
            panic!("only two outputs supported for BlockCopy");
        }
    }






    #[inline(always)]
    fn forward_backward(&mut self, 
                        further_blocks: &mut [Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        update:bool) {
        debug_assert!(self.input_offset != usize::MAX);
        debug_assert!(self.output_offset != usize::MAX);
        debug_assert!(self.num_inputs > 0);
        
        unsafe {
            // plain copy from input to output
            pb.tape.copy_within(self.input_offset .. (self.input_offset + self.num_inputs), self.output_offset);
                        
            //pb.tapes[self.output_tape_index as usize].extend_from_slice(pb.tapes.get_unchecked(self.input_tape_index as usize).get_unchecked(input_tape_start .. input_tape_start + self.num_inputs as usize));
            if further_blocks.len() > 0 {
                let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
                next_regressor[0].forward_backward(further_blocks, fb, pb, update);
            }
            if update {
                // Sum up the gradients from output to input
                for i in 0..self.num_inputs as usize {
                    let w = *pb.tape.get_unchecked(self.output_offset + i);
//                    println!("AAAAAA: {}, initial: {}", w, *(pb.tapes.get_unchecked_mut(self.input_tape_index as usize)).get_unchecked_mut(input_tape_start + i));
                    *pb.tape.get_unchecked_mut(self.input_offset + i) += w;
                }

            }
            // The only exit point
            return
            
        } // unsafe end
    }
    
    fn forward(&self, further_blocks: &[Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        ) {
            // plain copy from input to output
            pb.tape.copy_within(self.input_offset .. (self.input_offset + self.num_inputs), self.output_offset);
                        
            //pb.tapes[self.output_tape_index as usize].extend_from_slice(pb.tapes.get_unchecked(self.input_tape_index as usize).get_unchecked(input_tape_start .. input_tape_start + self.num_inputs as usize));
            if further_blocks.len() > 0 {
                let (next_regressor, further_blocks) = further_blocks.split_at(1);
                next_regressor[0].forward(further_blocks, fb, pb);
            }
    }
    
}




pub struct BlockJoin {    
    pub num_inputs: usize,
    pub input_offset: usize,
    pub output_offset: usize,
}


pub fn new_join_block(bg: &mut graph::BlockGraph,
                       inputs: Vec<graph::BlockPtrOutput>,
                       ) -> Result<graph::BlockPtrOutput, Box<dyn Error>> {
    let num_inputs = bg.get_num_output_values(inputs.iter().collect());
    assert!(num_inputs != 0);

    let mut block = Box::new(BlockJoin {
        output_offset: usize::MAX,
        input_offset: usize::MAX,
        num_inputs: num_inputs,
    });
    let mut block_outputs = bg.add_node(block, inputs)?;
    assert_eq!(block_outputs.len(), 1);
    Ok(block_outputs.pop().unwrap())
}

impl BlockTrait for BlockJoin {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
    
    fn get_block_type(&self) -> graph::BlockType {graph::BlockType::Join}  

    fn get_num_output_slots(&self) -> usize {1}
    
    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() == 0);
        self.num_inputs
    }

    fn get_input_offset(&mut self, input: graph::InputSlot) -> Result<usize, Box<dyn Error>> {
        assert!(input.get_input_index() <= 1);
        Ok(self.input_offset)
    }

    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        assert!(input.get_input_index() <= 1);
        if input.get_input_index() == 0 {
            self.input_offset = offset;
        } else if input.get_input_index() == 1 {
            assert!(self.input_offset <= offset, "Output 1, error 1: Input offset: {}, num_inputs: {}, offset: {}", self.input_offset, self.num_inputs, offset);
            assert!(self.input_offset + self.num_inputs >= offset, "Output 1, error 2: Input offset: {}, num_inputs: {}, offset: {}", self.input_offset, self.num_inputs, offset);
        }
        
    } 

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize) {
        assert!(output.get_output_index() == 0);
        self.output_offset = offset;
    }

    // WARNING: These two functions are automatically removed from the graph when executing, since they are a no-op
    #[inline(always)]
    fn forward_backward(&mut self, 
                        further_blocks: &mut [Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        update:bool) {
        debug_assert!(self.input_offset != usize::MAX);
        debug_assert!(self.output_offset != usize::MAX);
        debug_assert!(self.num_inputs > 0);
        
        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
            next_regressor[0].forward_backward(further_blocks, fb, pb, update);
        }
    }
    
    fn forward(&self, further_blocks: &[Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        ) {
        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at(1);
            next_regressor[0].forward(further_blocks, fb, pb);
        }

    }
    
}


pub struct BlockSum {    
    pub num_inputs: usize,
    pub input_offset: usize,
    pub output_offset: usize,

}


fn new_sum_without_weights(
                          num_inputs: usize, 
                          ) -> Result<Box<dyn BlockTrait>, Box<dyn Error>> {
    assert!(num_inputs > 0);
    let mut rg = BlockSum {
        output_offset: usize::MAX,
        input_offset: usize::MAX,
        num_inputs: num_inputs,
    };
    Ok(Box::new(rg))
}




pub fn new_sum_block(   bg: &mut graph::BlockGraph, 
                        input: graph::BlockPtrOutput,
                        ) -> Result<graph::BlockPtrOutput, Box<dyn Error>> {    
    let num_inputs = bg.get_num_output_values(vec![&input]);
    let block = new_sum_without_weights(num_inputs).unwrap(); 
    let mut block_outputs = bg.add_node(block, vec![input]).unwrap();
    assert_eq!(block_outputs.len(), 1);
    Ok(block_outputs.pop().unwrap())
}







impl BlockTrait for BlockSum {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn get_num_output_slots(&self) -> usize {1}   

    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() == 0);
        1
    }

    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        assert!(input.get_input_index() == 0);
        self.input_offset = offset;
    }

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize)  {
        assert!(output.get_output_index() == 0);
        self.output_offset = offset;
    }

    #[inline(always)]
    fn forward_backward(&mut self, 
                        further_blocks: &mut [Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        update:bool) {
        debug_assert!(self.num_inputs > 0);
        debug_assert!(self.output_offset != usize::MAX);
        debug_assert!(self.input_offset != usize::MAX);
        
        let wsum:f32 = pb.tape[self.input_offset .. (self.input_offset + self.num_inputs as usize)].iter().sum();
        pb.tape[self.output_offset as usize] = wsum;
        
        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
            next_regressor[0].forward_backward(further_blocks, fb, pb, update);
        }

        let general_gradient = pb.tape[self.output_offset];
        if update {
            pb.tape[self.input_offset .. (self.input_offset + self.num_inputs as usize)].fill(general_gradient);
        } // unsafe end
    }
    
    fn forward(		    &self, 		
                            further_blocks: &[Box<dyn BlockTrait>], 
                            fb: &feature_buffer::FeatureBuffer,
                            pb: &mut port_buffer::PortBuffer, 
                           ) {
        debug_assert!(self.num_inputs > 0);
        debug_assert!(self.output_offset != usize::MAX);
        debug_assert!(self.input_offset != usize::MAX);
        
        let wsum:f32 = pb.tape[self.input_offset .. (self.input_offset + self.num_inputs as usize)].iter().sum();
        pb.tape[self.output_offset as usize] = wsum;

        if further_blocks.len() > 0 {
            let (next_regressor, further_blocks) = further_blocks.split_at(1);
            next_regressor[0].forward(further_blocks, fb, pb);
        }
    }
}












// From a square only keep weights that are on the lower left triangle + diagonal
pub struct BlockTriangle {    
    pub square_width: usize,
    pub num_inputs: usize,
    pub num_outputs: usize,
    pub input_offset: usize,
    pub output_offset: usize,
}


pub fn new_triangle_block(bg: &mut graph::BlockGraph,
                        input: graph::BlockPtrOutput,
                       ) -> Result<graph::BlockPtrOutput, Box<dyn Error>> {
    let num_inputs = bg.get_num_output_values(vec![&input]);
    assert!(num_inputs != 0);

    let num_inputs_sqrt = (num_inputs as f32).sqrt() as usize;
    if num_inputs_sqrt * num_inputs_sqrt != num_inputs {
        panic!("Triangle has to have number of inputs as square number, instead we have: {} whose square is {}", num_inputs, num_inputs_sqrt);
    }
    let square_width = num_inputs_sqrt;
    let num_outputs = square_width * (square_width + 1) / 2;
    let mut block = Box::new(BlockTriangle {
        output_offset: usize::MAX,
        input_offset: usize::MAX,
        num_inputs: square_width * square_width,
        num_outputs: num_outputs,
        square_width: square_width,
    });
    let mut block_outputs = bg.add_node(block, vec![input])?;
    assert_eq!(block_outputs.len(), 1);
    Ok(block_outputs.pop().unwrap())
}


pub fn borrow_two(i: &mut Vec<f32>, 
                  start1: usize, len1: usize, 
                  start2: usize, len2: usize) -> (&mut [f32], &mut [f32]) {
    debug_assert!((start1 >= start2+len2) || (start2 >= start1+len1), "start1: {}, len1: {}, start2: {}, len2 {}", start1, len1, start2, len2);
    
    unsafe {
    if start2 > start1 {
        let (rest, second) = i.split_at_mut(start2);
        let (rest, first) = rest.split_at_mut(start1);
        return (first.get_unchecked_mut(0..len1), second.get_unchecked_mut(0..len2))
//        return (&mut first[0..len1], &mut second[0..len2]);
    } else {
        let (rest, first) = i.split_at_mut(start1);
        let (rest, second) = rest.split_at_mut(start2);
        return (first.get_unchecked_mut(0..len1), second.get_unchecked_mut(0..len2))
//        return (&mut first[0..len1], &mut second[0..len2]);
    
    }
    }
    
} 



impl BlockTrait for BlockTriangle

 {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn allocate_and_init_weights(&mut self, mi: &model_instance::ModelInstance) {
    }

    fn get_num_output_slots(&self) -> usize {1}

    fn get_num_output_values(&self, output: graph::OutputSlot) -> usize {
        assert!(output.get_output_index() == 0);
        self.num_outputs
    }

    fn set_input_offset(&mut self, input: graph::InputSlot, offset: usize)  {
        assert!(input.get_input_index() == 0);
        self.input_offset = offset;
    }

    fn set_output_offset(&mut self, output: graph::OutputSlot, offset: usize) {
        assert!(output.get_output_index() == 0);
        self.output_offset = offset;
    }






    #[inline(always)]
    fn forward_backward(&mut self, 
                        further_blocks: &mut [Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        update:bool) {
        debug_assert!(self.input_offset != usize::MAX);
        debug_assert!(self.output_offset != usize::MAX);
        debug_assert!(self.num_inputs > 0);
        
        unsafe {
            {
//            let input_tape = pb.tape.get_unchecked(self.input_offset..(self.input_offset + self.num_inputs as usize));
//            let output_tape = pb.tape.get_unchecked_mut(self.output_offset..(self.output_offset + self.num_outputs as usize));
                let (input_tape, output_tape) = borrow_two(&mut pb.tape, 
                                                            self.input_offset, self.num_inputs,
                                                            self.output_offset, self.num_outputs); 
      
      
                let mut output_index: usize = 0;
                for i in 0..self.square_width {
//                    println!("AAAAA i: {}", i);
                    for j in 0..i+1 {
                        let input = input_tape[i * self.square_width + j];
  //                      println!("Output index: {}, i: {}, j: {}, square_width: {}", output_index, i, j, self.square_width);
                        output_tape[output_index] = input_tape[i * self.square_width + j];
                        output_index += 1;
                    }
                }
            }
            
            //pb.tapes[self.output_tape_index as usize].extend_from_slice(pb.tapes.get_unchecked(self.input_tape_index as usize).get_unchecked(input_tape_start .. input_tape_start + self.num_inputs as usize));
            if further_blocks.len() > 0 {
                let (next_regressor, further_blocks) = further_blocks.split_at_mut(1);
                next_regressor[0].forward_backward(further_blocks, fb, pb, update);
            }
            if update {
                let (input_tape, output_tape) = borrow_two(&mut pb.tape, 
                                                            self.input_offset, self.num_inputs,
                                                            self.output_offset, self.num_outputs); 

                let mut output_index: usize = 0;
                for i in 0..self.square_width {
                    for j in 0..i+1 {
                        input_tape[i * self.square_width + j] = output_tape[output_index];
                        input_tape[j * self.square_width + i] = output_tape[output_index];
                        output_index += 1;
                    }
                }
            }

        } // unsafe end
    }
    
    fn forward(&self, further_blocks: &[Box<dyn BlockTrait>], 
                        fb: &feature_buffer::FeatureBuffer, 
                        pb: &mut port_buffer::PortBuffer, 
                        ) {
            // plain copy from input to output
            pb.tape.copy_within(self.input_offset .. (self.input_offset + self.num_inputs), self.output_offset);
                        
            //pb.tapes[self.output_tape_index as usize].extend_from_slice(pb.tapes.get_unchecked(self.input_tape_index as usize).get_unchecked(input_tape_start .. input_tape_start + self.num_inputs as usize));
            if further_blocks.len() > 0 {
                let (next_regressor, further_blocks) = further_blocks.split_at(1);
                next_regressor[0].forward(further_blocks, fb, pb);
            }
    }
    
}




mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use crate::block_loss_functions;
    use crate::block_misc;
    use crate::model_instance::Optimizer;
    use crate::feature_buffer;
    use crate::feature_buffer::HashAndValueAndSeq;
    use crate::vwmap;
    use crate::block_helpers::{slearn2, spredict2};
    use crate::graph;
    use crate::graph::{BlockGraph};
    use crate::block_misc::Observe;
    use crate::assert_epsilon;

    fn fb_vec() -> feature_buffer::FeatureBuffer {
        feature_buffer::FeatureBuffer {
                    label: 0.0,
                    example_importance: 1.0,
                    example_number: 0,
                    lr_buffer: Vec::new(),
                    ffm_buffer: Vec::new(),
                    ffm_fields_count: 0,
        }
    }


    #[test]
    fn test_sum_block() {
        let mut mi = model_instance::ModelInstance::new_empty().unwrap();        
        mi.learning_rate = 0.1;
        mi.power_t = 0.0;
        mi.optimizer = Optimizer::SGD;
        
       
        let mut bg = BlockGraph::new();
        let input_block = block_misc::new_const_block(&mut bg, vec![2.0, 3.0]).unwrap();
        let sum_block = new_sum_block(&mut bg, input_block).unwrap();
        let observe_block = block_misc::new_observe_block(&mut bg, sum_block, Observe::Forward, Some(1.0)).unwrap();
        bg.schedule();
        bg.allocate_and_init_weights(&mi);
        
        let mut pb = bg.new_port_buffer();
        let fb = fb_vec();
        assert_epsilon!(slearn2  (&mut bg, &fb, &mut pb, true), 5.0);
        
        

    }


    #[test]
    fn test_triangle_block() {
        let mut mi = model_instance::ModelInstance::new_empty().unwrap();        
        mi.learning_rate = 0.1;
        mi.power_t = 0.0;
        mi.optimizer = Optimizer::SGD;
        
       
        let mut bg = BlockGraph::new();
        let input_block = block_misc::new_const_block(&mut bg, vec![2.0, 3.0, 4.0, 5.0]).unwrap();
        let observe_block_backward = block_misc::new_observe_block(&mut bg, input_block, Observe::Backward, None).unwrap();
        let triangle_block = new_triangle_block(&mut bg, observe_block_backward).unwrap();
        let observe_block_forward = block_misc::new_observe_block(&mut bg, triangle_block, Observe::Forward, None).unwrap();
        bg.schedule();
        bg.allocate_and_init_weights(&mi);
        
        let mut pb = bg.new_port_buffer();
        let fb = fb_vec();
        slearn2  (&mut bg, &fb, &mut pb, true);
        assert_eq!(pb.observations, vec![2.0, 4.0, 5.0,			// forward part
                                         2.0, 4.0, 4.0, 5.0]);		// backward part -- 3.0 gets turned into 4.0 since that is its transpose
        

    }




}




















